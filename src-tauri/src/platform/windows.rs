//! Windows backend: registry soft-flag + external mutex release + shell launch.
//! Zero injection into WXWork.exe.

use crate::models::{LaunchOptions, LaunchResult};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;
use windows::Win32::Foundation::{
    CloseHandle, DuplicateHandle, BOOL, DUPLICATE_CLOSE_SOURCE, DUPLICATE_SAME_ACCESS, HANDLE,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE, PROCESS_QUERY_LIMITED_INFORMATION,
};
use winreg::enums::*;
use winreg::RegKey;

const MUTEX_HINTS: &[&str] = &[
    "Tencent.WeWork.ExclusiveObject",
    "Tencent.WeWork.ExclusiveObjectInstance",
    "Tencent.WeWork.Exclusive",
    "Tencent.WeWork.Instance",
    "WeWorkExclusive",
    "WXWorkExclusive",
    "WXWork_Exclusive",
    "WeWork_ExclusiveObject",
];

pub fn resolve_app_path(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    if let Some(p) = from_registry() {
        return Some(p);
    }

    for candidate in [
        r"C:\Program Files (x86)\WXWork\WXWork.exe",
        r"C:\Program Files\WXWork\WXWork.exe",
        r"D:\Program Files (x86)\WXWork\WXWork.exe",
        r"D:\Program Files\WXWork\WXWork.exe",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn from_registry() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(r"Software\Tencent\WXWork").ok()?;
    let install: String = key
        .get_value("Executable")
        .or_else(|_| key.get_value::<String, _>("InstallPath"))
        .ok()?;
    let path = if install.to_lowercase().ends_with(".exe") {
        PathBuf::from(install)
    } else {
        PathBuf::from(install.trim_end_matches(['\\', '/'])).join("WXWork.exe")
    };
    path.exists().then_some(path)
}

fn try_enable_multi_instances(max: u32) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = match hkcu.create_subkey(r"Software\Tencent\WXWork") {
        Ok(v) => v,
        Err(_) => return false,
    };
    key.set_value("multi_instances", &max).is_ok()
}

pub fn prepare_next_instance(opts: &LaunchOptions) -> Result<String, String> {
    if opts.prefer_registry {
        let _ = try_enable_multi_instances(crate::models::MAX_SLOTS as u32);
    }

    let dbg = enable_debug_privilege();
    let elevated = is_elevated();
    let mut messages = Vec::new();
    if !elevated {
        messages.push("建议以管理员运行（否则可能无法释放互斥体）".into());
    }
    if !dbg {
        messages.push("SeDebugPrivilege 未启用".into());
    }

    // Fast path: nothing running → skip expensive handle scan entirely.
    let existing = wecom_main_pids();
    if existing.is_empty() {
        messages.push("无运行中企微，跳过互斥扫描".into());
        return Ok(messages.join(" | "));
    }

    // At most one quick release before launch (full system handle walk is costly).
    match release_wecom_mutexes_budgeted(std::time::Duration::from_millis(2000)) {
        Ok(msg) => messages.push(msg),
        Err(e) => messages.push(format!("预释放: {e}")),
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    Ok(messages.join(" | "))
}

fn slot_state_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("WeComLauncher").join("slot-pids.json")
}

/// Slots whose last launched PID is still a live WeCom process.
pub fn busy_slot_indices() -> Vec<u8> {
    let live: std::collections::HashSet<u32> = crate::platform::list_wecom_pids().into_iter().collect();
    let mut map = load_slot_pids();
    let mut busy = Vec::new();
    let mut changed = false;
    for i in 1..=crate::models::MAX_SLOTS {
        if let Some(pid) = map.get(&i).copied() {
            if live.contains(&pid) {
                busy.push(i);
            } else {
                map.remove(&i);
                changed = true;
            }
        }
    }
    if changed {
        let _ = save_slot_pids(&map);
    }
    busy
}

pub fn note_slot_pid(index: u8, pid: Option<u32>) {
    let mut map = load_slot_pids();
    match pid {
        Some(p) => {
            map.insert(index, p);
        }
        None => {
            map.remove(&index);
        }
    }
    let _ = save_slot_pids(&map);
}

pub fn clear_slot_pid_state() {
    let path = slot_state_path();
    let _ = fs::remove_file(path);
}

fn load_slot_pids() -> std::collections::HashMap<u8, u32> {
    let path = slot_state_path();
    let Ok(bytes) = fs::read(&path) else {
        return std::collections::HashMap::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save_slot_pids(map: &std::collections::HashMap<u8, u32>) -> Result<(), String> {
    let path = slot_state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(map).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn spawn_instance(
    app_path: &Path,
    index: u8,
    opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    if opts.windows_safe_mode {
        return spawn_as_local_user(app_path, index, opts);
    }

    let workdir = app_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Direct CreateProcess first (we need a real PID to release its mutex later).
    // Shell `start` is fallback only.
    let started = spawn_direct(app_path, &workdir).or_else(|_| spawn_via_shell_start(app_path, &workdir));

    match started {
        Ok(pid) => {
            // Brief wait then ONE budgeted mutex pass (old 3× full scans could hang for minutes).
            std::thread::sleep(std::time::Duration::from_millis(400));
            let _ = enable_debug_privilege();
            let post = release_wecom_mutexes_budgeted(std::time::Duration::from_millis(2500))
                .unwrap_or_else(|e| format!("启动后释放跳过: {e}"));

            if let Some(p) = pid {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if !process_still_running(p) {
                    return Ok(LaunchResult {
                        success: false,
                        pid: Some(p),
                        message: format!(
                            "实例 #{index} PID={p} 启动后退出。请以管理员运行，并先完全退出企微后再试。| {post}"
                        ),
                        index,
                    });
                }
            }

            Ok(LaunchResult {
                success: true,
                pid,
                message: format!(
                    "已启动实例 #{index}{} | {post}",
                    pid.map(|p| format!(" PID={p}")).unwrap_or_default()
                ),
                index,
            })
        }
        Err(e) => Ok(LaunchResult {
            success: false,
            pid: None,
            message: format!("启动失败: {e}"),
            index,
        }),
    }
}

fn process_still_running(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code).is_ok();
        let _ = CloseHandle(handle);
        ok && code == STILL_ACTIVE
    }
}

fn spawn_via_shell_start(app_path: &Path, workdir: &Path) -> Result<Option<u32>, String> {
    // cmd /C start "" /D "workdir" "exe"
    let status = Command::new("cmd")
        .args([
            "/C",
            "start",
            "",
            "/D",
            &workdir.to_string_lossy(),
            &app_path.to_string_lossy(),
        ])
        .spawn()
        .map_err(|e| e.to_string())?;
    // `start` returns immediately; PID of cmd is not the WeCom PID.
    let _ = status;
    Ok(None)
}

fn spawn_direct(app_path: &Path, workdir: &Path) -> Result<Option<u32>, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    let child = Command::new(app_path)
        .current_dir(workdir)
        .creation_flags(CREATE_NEW_CONSOLE | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(Some(child.id()))
}

fn enable_debug_privilege() -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::core::PCWSTR;

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return false;
        }

        let mut luid = LUID::default();
        let name: Vec<u16> = "SeDebugPrivilege"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        if LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(name.as_ptr()), &mut luid).is_err() {
            let _ = CloseHandle(token);
            return false;
        }

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let ok = AdjustTokenPrivileges(token, false, Some(&mut tp), 0, None, None).is_ok();
        let _ = CloseHandle(token);
        ok
    }
}

/// Create local users WeComSlot1..N if missing. Requires administrator.
pub fn ensure_safe_mode_users(
    count: u8,
    password: &str,
    prefix: &str,
) -> Result<crate::models::SafeModePrepareResult, String> {
    use crate::models::{SafeModePrepareResult, MAX_SLOTS};
    use crate::policy::clamp_count;

    if password.trim().len() < 8 {
        return Err("安全模式密码至少 8 位（需满足 Windows 密码策略）".into());
    }

    let count = clamp_count(count);
    let prefix = if prefix.trim().is_empty() {
        crate::models::DEFAULT_SAFE_USER_PREFIX
    } else {
        prefix.trim()
    };

    let mut created = Vec::new();
    let mut already_existed = Vec::new();
    let mut failed = Vec::new();

    for i in 1..=count.min(MAX_SLOTS) {
        let username = format!("{prefix}{i}");
        if local_user_exists(&username) {
            already_existed.push(username);
            continue;
        }
        match create_local_user(&username, password) {
            Ok(()) => created.push(username),
            Err(e) => failed.push(format!("{username}: {e}")),
        }
    }

    let message = if failed.is_empty() {
        format!(
            "安全模式用户就绪：新建 {}，已存在 {}。请以管理员运行启动器。",
            created.len(),
            already_existed.len()
        )
    } else {
        format!(
            "部分失败（通常需要管理员权限）: {}",
            failed.join("; ")
        )
    };

    Ok(SafeModePrepareResult {
        created,
        already_existed,
        failed,
        message,
    })
}

pub fn list_safe_mode_users(count: u8, prefix: &str) -> Vec<crate::models::SafeModeUserStatus> {
    use crate::models::{SafeModeUserStatus, DEFAULT_SAFE_USER_PREFIX, MAX_SLOTS};
    use crate::policy::clamp_count;

    let count = clamp_count(count).min(MAX_SLOTS);
    let prefix = if prefix.trim().is_empty() {
        DEFAULT_SAFE_USER_PREFIX
    } else {
        prefix.trim()
    };

    (1..=count)
        .map(|index| {
            let username = format!("{prefix}{index}");
            let exists = local_user_exists(&username);
            SafeModeUserStatus {
                index,
                username,
                exists,
            }
        })
        .collect()
}

/// `net session` succeeds only when the process is elevated.
pub fn is_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn check_safe_mode_health(
    count: u8,
    prefix: &str,
    password: Option<&str>,
) -> crate::models::SafeModeHealth {
    use crate::models::SafeModeHealth;
    use crate::policy::clamp_count;

    let count = clamp_count(count);
    let users = list_safe_mode_users(count, prefix);
    let missing_users: Vec<String> = users
        .iter()
        .filter(|u| !u.exists)
        .map(|u| u.username.clone())
        .collect();

    let is_admin = is_elevated();
    let password_ok = password
        .map(str::trim)
        .filter(|s| s.len() >= 8)
        .is_some();

    let mut warnings = Vec::new();
    if !is_admin {
        warnings.push("当前未以管理员运行：准备本地用户可能失败，建议右键「以管理员身份运行」。".into());
    }
    if !password_ok {
        warnings.push("本地用户密码未填或不足 8 位。".into());
    }
    if !missing_users.is_empty() {
        warnings.push(format!(
            "缺少本地用户：{}。请先点「准备本地用户」。",
            missing_users.join(", ")
        ));
    }

    let ready = missing_users.is_empty() && password_ok;
    let summary = if ready {
        format!(
            "安全模式校验通过：{}/{} 用户已就绪{}",
            users.len() - missing_users.len(),
            users.len(),
            if is_admin { "，管理员权限正常" } else { "（非管理员，启动通常仍可用）" }
        )
    } else {
        format!("安全模式未就绪：{}", warnings.join(" "))
    };

    SafeModeHealth {
        platform_ok: true,
        is_admin,
        password_ok,
        users,
        missing_users,
        ready,
        warnings,
        summary,
    }
}

fn local_user_exists(username: &str) -> bool {
    let output = Command::new("net")
        .args(["user", username])
        .output();
    matches!(output, Ok(o) if o.status.success())
}

fn create_local_user(username: &str, password: &str) -> Result<(), String> {
    // net user is the most portable admin path; requires elevated launcher.
    let add = Command::new("net")
        .args(["user", username, password, "/add", "/fullnamepasswordchg:yes", "/expires:never"])
        .output()
        .map_err(|e| format!("执行 net user 失败: {e}"))?;

    if !add.status.success() {
        let stderr = String::from_utf8_lossy(&add.stderr);
        let stdout = String::from_utf8_lossy(&add.stdout);
        return Err(format!(
            "创建用户失败（请以管理员运行）: {} {}",
            stdout.trim(),
            stderr.trim()
        ));
    }

    // Ensure Users group membership (usually automatic, but be explicit).
    let _ = Command::new("net")
        .args(["localgroup", "Users", username, "/add"])
        .output();

    Ok(())
}

fn spawn_as_local_user(
    app_path: &Path,
    index: u8,
    opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        CreateProcessWithLogonW, CREATE_NEW_CONSOLE, CREATE_UNICODE_ENVIRONMENT, LOGON_WITH_PROFILE,
        PROCESS_INFORMATION, STARTUPINFOW,
    };

    let password = opts
        .safe_mode_password
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "安全模式需要填写本地用户密码".to_string())?;

    let username = opts.safe_username(index);
    if !local_user_exists(&username) {
        return Ok(LaunchResult {
            success: false,
            pid: None,
            message: format!(
                "本地用户 {username} 不存在。请先点「准备本地用户」（需管理员）"
            ),
            index,
        });
    }

    let workdir = app_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let mut user_w = to_wide(&username);
    let mut domain_w = to_wide(".");
    let mut pass_w = to_wide(password);
    let mut app_w = to_wide(&app_path.to_string_lossy());
    // Command line must be mutable for CreateProcess*
    let mut cmd_w = to_wide(&format!("\"{}\"", app_path.to_string_lossy()));
    let mut dir_w = to_wide(&workdir.to_string_lossy());

    unsafe {
        let mut si = STARTUPINFOW::default();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut pi = PROCESS_INFORMATION::default();

        let ok = CreateProcessWithLogonW(
            PCWSTR(user_w.as_ptr()),
            PCWSTR(domain_w.as_ptr()),
            PCWSTR(pass_w.as_ptr()),
            LOGON_WITH_PROFILE,
            PCWSTR(app_w.as_ptr()),
            PWSTR(cmd_w.as_mut_ptr()),
            CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE,
            None,
            PCWSTR(dir_w.as_ptr()),
            &si,
            &mut pi,
        );

        // Best-effort wipe password buffer
        for b in pass_w.iter_mut() {
            *b = 0;
        }

        match ok {
            Ok(()) => {
                let pid = pi.dwProcessId;
                let _ = CloseHandle(pi.hThread);
                let _ = CloseHandle(pi.hProcess);
                Ok(LaunchResult {
                    success: true,
                    pid: Some(pid),
                    message: format!(
                        "安全模式已启动 #{index} 用户={username} PID={pid}"
                    ),
                    index,
                })
            }
            Err(e) => Ok(LaunchResult {
                success: false,
                pid: None,
                message: format!(
                    "以用户 {username} 启动失败: {e}（检查密码、用户是否存在、是否被策略禁止）"
                ),
                index,
            }),
        }
    }
}

/// Prefer main WXWork.exe PIDs — helpers have huge handle tables and are irrelevant for mutex.
fn wecom_main_pids() -> Vec<u32> {
    use sysinfo::{ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut pids = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();
        let exe = process
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let hay = format!("{name} {exe}").to_ascii_lowercase();
        if hay.contains("wecom-multi-launcher") || hay.contains("wecom launcher") {
            continue;
        }
        let is_main = name.eq_ignore_ascii_case("WXWork.exe")
            || name.eq_ignore_ascii_case("WXWork")
            || exe.to_ascii_lowercase().ends_with("\\wxwork.exe")
            || exe.to_ascii_lowercase().ends_with("/wxwork.exe");
        if is_main {
            pids.push(pid.as_u32());
        }
    }
    pids.sort_unstable();
    pids.dedup();
    // Fallback: any WeCom-matched process if name filter found nothing.
    if pids.is_empty() {
        return crate::platform::list_wecom_pids();
    }
    pids
}

fn release_wecom_mutexes() -> Result<String, String> {
    release_wecom_mutexes_budgeted(std::time::Duration::from_millis(2500))
}

fn release_wecom_mutexes_budgeted(budget: std::time::Duration) -> Result<String, String> {
    let pids = wecom_main_pids();
    if pids.is_empty() {
        return Ok("未发现运行中的企业微信，可直接启动".into());
    }

    let deadline = std::time::Instant::now() + budget;
    let mutant_ty = mutant_type_index();
    let mut closed = 0usize;
    let mut errors = Vec::new();

    for pid in pids {
        if std::time::Instant::now() >= deadline {
            break;
        }
        let remain = deadline.saturating_duration_since(std::time::Instant::now());
        match close_mutex_handles_for_pid(pid, mutant_ty, remain) {
            Ok(n) => closed += n,
            Err(e) => errors.push(format!("PID {pid}: {e}")),
        }
    }

    if closed == 0 && !errors.is_empty() {
        return Err(format!(
            "释放互斥体失败（可尝试以管理员运行）: {}",
            errors.join("; ")
        ));
    }

    if closed > 0 {
        Ok(format!("已释放 {closed} 个互斥句柄"))
    } else {
        Ok("未匹配到互斥句柄（可能已释放或版本更名）".into())
    }
}

fn close_mutex_handles_for_pid(
    pid: u32,
    mutant_ty: Option<u16>,
    budget: std::time::Duration,
) -> Result<usize, String> {
    let deadline = std::time::Instant::now() + budget;
    unsafe {
        let process = OpenProcess(
            PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
        .map_err(|e| format!("OpenProcess 失败: {e}（需要管理员权限？）"))?;

        // Handle-table walk itself can be slow — time-box it.
        let enum_budget = std::cmp::min(budget, std::time::Duration::from_millis(1500));
        let entries = enumerate_handle_entries_timed(pid, enum_budget)?;
        let mut closed = 0usize;
        let current = GetCurrentProcess();
        let mut name_queries = 0usize;
        const MAX_NAME_QUERIES: usize = 120;

        for (handle_value, type_index) in entries {
            if std::time::Instant::now() >= deadline {
                break;
            }
            // Only inspect Mutant handles when we know the type index.
            if let Some(m) = mutant_ty {
                if type_index != m {
                    continue;
                }
            } else {
                // No type filter available — hard-cap name queries to avoid multi-minute hangs.
                if name_queries >= MAX_NAME_QUERIES {
                    break;
                }
            }

            name_queries += 1;
            let Some(name) = query_object_name_timed(process, handle_value, 30) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if !MUTEX_HINTS
                .iter()
                .any(|h| lower.contains(&h.to_ascii_lowercase()))
            {
                continue;
            }

            let mut local = HANDLE::default();
            if DuplicateHandle(
                process,
                HANDLE(handle_value as *mut _),
                current,
                &mut local,
                0,
                false,
                DUPLICATE_CLOSE_SOURCE,
            )
            .is_ok()
            {
                if !local.is_invalid() {
                    let _ = CloseHandle(local);
                }
                closed += 1;
            }
        }

        let _ = CloseHandle(process);
        Ok(closed)
    }
}

const SystemExtendedHandleInformation: u32 = 64;
const STATUS_INFO_LENGTH_MISMATCH: i32 = -1073741820; // 0xC0000004
const ObjectNameInformation: u32 = 1;

#[repr(C)]
struct SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX {
    object: usize,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_back_trace_index: u16,
    object_type_index: u16,
    handle_attributes: u32,
    reserved: u32,
}

#[repr(C)]
struct UNICODE_STRING {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQuerySystemInformation(
        class: u32,
        info: *mut u8,
        length: u32,
        return_length: *mut u32,
    ) -> i32;

    fn NtQueryObject(
        handle: HANDLE,
        info_class: u32,
        info: *mut u8,
        length: u32,
        return_length: *mut u32,
    ) -> i32;
}

fn mutant_type_index() -> Option<u16> {
    use std::sync::OnceLock;
    static IDX: OnceLock<Option<u16>> = OnceLock::new();
    *IDX.get_or_init(|| unsafe { probe_mutant_type_index() })
}

unsafe fn probe_mutant_type_index() -> Option<u16> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::CreateMutexW;

    let name: Vec<u16> = "Local\\WeComLauncher_MutantProbe\0"
        .encode_utf16()
        .collect();
    let mutex = CreateMutexW(None, false, PCWSTR(name.as_ptr())).ok()?;
    let self_pid = std::process::id();
    let target = mutex.0 as usize;
    let mut found = None;
    if let Ok(entries) = enumerate_handle_entries(self_pid) {
        for (hv, ty) in entries {
            // Handle values are often compared in the low 32/16 bits depending on OS.
            if hv == target || hv == (target & 0xffff) || hv == (target & 0xffff_ffff) {
                found = Some(ty);
                break;
            }
        }
    }
    let _ = CloseHandle(mutex);
    found
}

unsafe fn enumerate_handle_entries(pid: u32) -> Result<Vec<(usize, u16)>, String> {
    let mut size = 1024 * 1024usize;
    for _ in 0..6 {
        let mut buf = vec![0u8; size];
        let mut ret = 0u32;
        let status = NtQuerySystemInformation(
            SystemExtendedHandleInformation,
            buf.as_mut_ptr(),
            buf.len() as u32,
            &mut ret,
        );
        if status == STATUS_INFO_LENGTH_MISMATCH {
            size = (ret as usize).saturating_mul(2).max(size * 2);
            // Soft cap ~64MB to avoid multi-minute allocations on huge systems.
            if size > 64 * 1024 * 1024 {
                return Err("系统句柄表过大，跳过本次扫描".into());
            }
            continue;
        }
        if status != 0 {
            return Err(format!("NtQuerySystemInformation 失败: 0x{status:X}"));
        }

        let number = ptr::read_unaligned(buf.as_ptr() as *const usize);
        let entry_size = mem::size_of::<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX>();
        let base = buf.as_ptr().add(mem::size_of::<usize>() * 2);
        let mut out = Vec::new();
        for i in 0..number {
            let entry_ptr = base.add(i * entry_size) as *const SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX;
            let entry = ptr::read_unaligned(entry_ptr);
            if entry.unique_process_id as u32 == pid {
                out.push((entry.handle_value, entry.object_type_index));
            }
        }
        return Ok(out);
    }
    Err("句柄枚举缓冲区不足".into())
}

fn enumerate_handle_entries_timed(
    pid: u32,
    budget: std::time::Duration,
) -> Result<Vec<(usize, u16)>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = unsafe { enumerate_handle_entries(pid) };
        let _ = tx.send(result);
    });
    match rx.recv_timeout(budget) {
        Ok(r) => r,
        Err(_) => Err(format!(
            "句柄枚举超时（{}ms），已跳过以免卡住启动",
            budget.as_millis()
        )),
    }
}

/// Query object name on a background thread with timeout — NtQueryObject can block forever.
fn query_object_name_timed(process: HANDLE, remote_handle: usize, timeout_ms: u64) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    // HANDLEs are just pointer-sized values; move copies into the worker.
    let process_bits = process.0 as usize;
    std::thread::spawn(move || {
        let process = HANDLE(process_bits as *mut _);
        let name = unsafe { query_object_name(process, remote_handle) };
        let _ = tx.send(name);
    });
    rx.recv_timeout(std::time::Duration::from_millis(timeout_ms))
        .ok()
        .flatten()
}

unsafe fn query_object_name(process: HANDLE, remote_handle: usize) -> Option<String> {
    let mut local = HANDLE::default();
    if DuplicateHandle(
        process,
        HANDLE(remote_handle as *mut _),
        GetCurrentProcess(),
        &mut local,
        0,
        false,
        DUPLICATE_SAME_ACCESS,
    )
    .is_err()
    {
        return None;
    }

    let mut len = 1024u32;
    let mut buf = vec![0u8; len as usize];
    let mut needed = 0u32;
    let mut status = NtQueryObject(
        local,
        ObjectNameInformation,
        buf.as_mut_ptr(),
        len,
        &mut needed,
    );
    if status == STATUS_INFO_LENGTH_MISMATCH {
        len = needed + 64;
        buf.resize(len as usize, 0);
        status = NtQueryObject(
            local,
            ObjectNameInformation,
            buf.as_mut_ptr(),
            len,
            &mut needed,
        );
    }
    let _ = CloseHandle(local);
    if status != 0 {
        return None;
    }

    let us = ptr::read_unaligned(buf.as_ptr() as *const UNICODE_STRING);
    if us.length == 0 || us.buffer.is_null() {
        return None;
    }
    let slice = std::slice::from_raw_parts(us.buffer, (us.length / 2) as usize);
    Some(String::from_utf16_lossy(slice))
}

#[allow(dead_code)]
fn _bool_to_bool(b: BOOL) -> bool {
    b.as_bool()
}

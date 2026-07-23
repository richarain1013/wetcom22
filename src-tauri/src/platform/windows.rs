//! Windows backend: registry soft-flag + external mutex release + shell launch.
//! Zero injection into WXWork.exe.

use crate::models::{LaunchOptions, LaunchResult};
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
    release_wecom_mutexes()
}

pub fn spawn_instance(
    app_path: &Path,
    index: u8,
    _opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    let workdir = app_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let mut cmd = Command::new(app_path);
    cmd.current_dir(&workdir);

    match cmd.spawn() {
        Ok(child) => {
            let pid = child.id();
            Ok(LaunchResult {
                success: true,
                pid: Some(pid),
                message: format!("已启动实例 #{index} PID={pid}"),
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

fn release_wecom_mutexes() -> Result<String, String> {
    let pids = crate::platform::list_wecom_pids();
    if pids.is_empty() {
        return Ok("未发现运行中的企业微信，可直接启动".into());
    }

    let mut closed = 0usize;
    let mut errors = Vec::new();

    for pid in pids {
        match close_mutex_handles_for_pid(pid) {
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

fn close_mutex_handles_for_pid(pid: u32) -> Result<usize, String> {
    unsafe {
        let process = OpenProcess(
            PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
        .map_err(|e| format!("OpenProcess 失败: {e}（需要管理员权限？）"))?;

        let handles = enumerate_handles(pid)?;
        let mut closed = 0usize;
        let current = GetCurrentProcess();

        for handle_value in handles {
            if let Some(name) = query_object_name(process, handle_value) {
                if MUTEX_HINTS
                    .iter()
                    .any(|h| name.to_ascii_lowercase().contains(&h.to_ascii_lowercase()))
                {
                    let mut local = HANDLE::default();
                    if DuplicateHandle(
                        process,
                        HANDLE(handle_value as *mut _),
                        current,
                        Some(&mut local),
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

unsafe fn enumerate_handles(pid: u32) -> Result<Vec<usize>, String> {
    let mut size = 1024 * 1024usize;
    for _ in 0..5 {
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
                out.push(entry.handle_value);
            }
        }
        return Ok(out);
    }
    Err("句柄枚举缓冲区不足".into())
}

unsafe fn query_object_name(process: HANDLE, remote_handle: usize) -> Option<String> {
    let mut local = HANDLE::default();
    if DuplicateHandle(
        process,
        HANDLE(remote_handle as *mut _),
        GetCurrentProcess(),
        Some(&mut local),
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

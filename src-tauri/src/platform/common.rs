use crate::models::MAX_SLOTS;
use std::collections::HashSet;
use sysinfo::{ProcessesToUpdate, System};

/// Process name fragments used to find WeCom across locales/versions.
pub fn wecom_process_matchers() -> &'static [&'static str] {
    &[
        "WXWork",
        "wxwork",
        "WeCom",
        "wecom",
        "企业微信",
        "WeChatWork",
        "wechatwork",
    ]
}

pub fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unsupported"
    }
}

pub fn list_wecom_pids() -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut pids = Vec::new();

    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        let exe = process
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let hay = format!("{name} {exe}");
        if wecom_process_matchers()
            .iter()
            .any(|m| hay.contains(m))
        {
            // Skip our own launcher if somehow named similarly
            if hay.contains("wecom-multi-launcher") || hay.contains("WeCom Launcher") {
                continue;
            }
            pids.push(pid.as_u32());
        }
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

pub fn kill_all_wecom() -> usize {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut n = 0usize;
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        let exe = process
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let hay = format!("{name} {exe}");
        if wecom_process_matchers()
            .iter()
            .any(|m| hay.contains(m))
            && !hay.contains("wecom-multi-launcher")
        {
            if process.kill() {
                n += 1;
            }
            let _ = pid;
        }
    }
    crate::platform::clear_slot_pid_state();
    n
}

/// Pick the next free account slots (1..=MAX_SLOTS), skipping ones already running.
/// 「新开 1 个」必须走这里，否则会反复启动同一 Bundle ID / 安全模式用户。
pub fn allocate_free_slots(need: u8) -> Result<Vec<u8>, String> {
    let need = need.clamp(1, MAX_SLOTS);
    let busy: HashSet<u8> = crate::platform::busy_slot_indices()
        .into_iter()
        .collect();
    let mut free = Vec::with_capacity(need as usize);
    for i in 1..=MAX_SLOTS {
        if !busy.contains(&i) {
            free.push(i);
            if free.len() == need as usize {
                return Ok(free);
            }
        }
    }
    Err(format!(
        "空闲槽位不足：需要 {need} 个，当前已占用 {} 个（最多 {MAX_SLOTS}）。请先关闭部分企微或点「全部关闭」。",
        busy.len()
    ))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub mod unsupported {
    use crate::models::{LaunchOptions, LaunchResult};
    use std::path::PathBuf;

    pub fn resolve_app_path(_override_path: Option<&str>) -> Option<PathBuf> {
        None
    }

    pub fn prepare_next_instance(_opts: &LaunchOptions) -> Result<String, String> {
        Err("当前平台不支持企业微信多开".into())
    }

    pub fn spawn_instance(
        _app_path: &std::path::Path,
        _index: u8,
        _opts: &LaunchOptions,
    ) -> Result<LaunchResult, String> {
        Err("当前平台不支持企业微信多开".into())
    }

    pub fn busy_slot_indices() -> Vec<u8> {
        Vec::new()
    }

    pub fn note_slot_pid(_index: u8, _pid: Option<u32>) {}

    pub fn clear_slot_pid_state() {}
}

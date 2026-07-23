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
    n
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
}

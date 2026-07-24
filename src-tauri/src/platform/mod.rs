mod common;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use common::unsupported::*;

pub use common::{current_platform, kill_all_wecom, list_wecom_pids};

#[cfg(not(target_os = "windows"))]
pub fn ensure_safe_mode_users(
    _count: u8,
    _password: &str,
    _prefix: &str,
) -> Result<crate::models::SafeModePrepareResult, String> {
    Err("安全模式（本地用户隔离）仅支持 Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn list_safe_mode_users(
    count: u8,
    prefix: &str,
) -> Vec<crate::models::SafeModeUserStatus> {
    use crate::models::{SafeModeUserStatus, DEFAULT_SAFE_USER_PREFIX, MAX_SLOTS};
    use crate::policy::clamp_count;

    let count = clamp_count(count).min(MAX_SLOTS);
    let prefix = if prefix.trim().is_empty() {
        DEFAULT_SAFE_USER_PREFIX
    } else {
        prefix.trim()
    };
    (1..=count)
        .map(|index| SafeModeUserStatus {
            index,
            username: format!("{prefix}{index}"),
            exists: false,
        })
        .collect()
}

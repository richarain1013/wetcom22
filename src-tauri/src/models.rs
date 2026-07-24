use serde::{Deserialize, Serialize};

pub const MAX_SLOTS: u8 = 10;
pub const DEFAULT_SAFE_USER_PREFIX: &str = "WeComSlot";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub count: u8,
    pub app_path: Option<String>,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    /// Windows: try HKCU multi_instances before mutex release.
    #[serde(default = "default_true")]
    pub prefer_registry: bool,
    /// macOS: clone .app with unique Bundle ID (better isolation for 8–10 accounts).
    #[serde(default = "default_true")]
    pub macos_clone_instances: bool,
    /// Windows Safe Mode: launch each instance as a dedicated local user.
    #[serde(default)]
    pub windows_safe_mode: bool,
    /// Password for WeComSlotN local users (required when safe mode is on).
    #[serde(default)]
    pub safe_mode_password: Option<String>,
    /// Local username prefix, default `WeComSlot` → WeComSlot1..WeComSlot10.
    #[serde(default = "default_safe_prefix")]
    pub safe_mode_user_prefix: String,
}

fn default_true() -> bool {
    true
}

fn default_safe_prefix() -> String {
    DEFAULT_SAFE_USER_PREFIX.to_string()
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            count: 1,
            app_path: None,
            min_delay_ms: 2500,
            max_delay_ms: 6000,
            prefer_registry: true,
            macos_clone_instances: true,
            windows_safe_mode: false,
            safe_mode_password: None,
            safe_mode_user_prefix: default_safe_prefix(),
        }
    }
}

impl LaunchOptions {
    pub fn safe_username(&self, index: u8) -> String {
        let prefix = if self.safe_mode_user_prefix.trim().is_empty() {
            DEFAULT_SAFE_USER_PREFIX
        } else {
            self.safe_mode_user_prefix.trim()
        };
        format!("{prefix}{index}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub success: bool,
    pub pid: Option<u32>,
    pub message: String,
    pub index: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub results: Vec<LaunchResult>,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub platform: String,
    pub resolved_path: Option<String>,
    pub running_count: usize,
    pub running_pids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeModePrepareRequest {
    pub count: u8,
    pub password: String,
    #[serde(default = "default_safe_prefix")]
    pub user_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeModeUserStatus {
    pub index: u8,
    pub username: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeModePrepareResult {
    pub created: Vec<String>,
    pub already_existed: Vec<String>,
    pub failed: Vec<String>,
    pub message: String,
}

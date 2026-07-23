use serde::{Deserialize, Serialize};

pub const MAX_SLOTS: u8 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub count: u8,
    pub app_path: Option<String>,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    /// Windows: try HKCU multi_instances before mutex release.
    pub prefer_registry: bool,
    /// macOS: clone .app with unique Bundle ID (better isolation for 8–10 accounts).
    /// If false, uses `open -n` on the original app (lighter, weaker isolation).
    pub macos_clone_instances: bool,
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
        }
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

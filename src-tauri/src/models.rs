use serde::{Deserialize, Serialize};

pub const MAX_SLOTS: u8 = 10;
pub const DEFAULT_SAFE_USER_PREFIX: &str = "WeComSlot";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AccountTier {
    Primary,
    #[default]
    Secondary,
    Test,
}

impl AccountTier {
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "primary" | "main" | "主号" => Self::Primary,
            "test" | "测试" => Self::Test,
            _ => Self::Secondary,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Test => "test",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Primary => "主号",
            Self::Secondary => "辅号",
            Self::Test => "测试",
        }
    }

    /// Delay window (ms) after launching a slot of this tier.
    pub fn delay_range(self) -> (u64, u64) {
        match self {
            Self::Primary => (5000, 10000),
            Self::Secondary => (2500, 6000),
            Self::Test => (1500, 3500),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub count: u8,
    pub app_path: Option<String>,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    #[serde(default = "default_true")]
    pub prefer_registry: bool,
    /// Deprecated API flag (ignored). macOS always uses hidden Application Support clones.
    #[serde(default)]
    pub macos_clone_instances: bool,
    #[serde(default)]
    pub windows_safe_mode: bool,
    #[serde(default)]
    pub safe_mode_password: Option<String>,
    #[serde(default = "default_safe_prefix")]
    pub safe_mode_user_prefix: String,
    /// Per-slot tiers aligned with indices 1..count (optional).
    #[serde(default)]
    pub slot_tiers: Vec<String>,
    /// When true, ignore global min/max and use tier delay ranges.
    #[serde(default)]
    pub use_tier_delays: bool,
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
            macos_clone_instances: false,
            windows_safe_mode: false,
            safe_mode_password: None,
            safe_mode_user_prefix: default_safe_prefix(),
            slot_tiers: Vec::new(),
            use_tier_delays: false,
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

    pub fn tier_for_index(&self, index: u8) -> AccountTier {
        let i = index.saturating_sub(1) as usize;
        self.slot_tiers
            .get(i)
            .map(|s| AccountTier::from_str_loose(s))
            .unwrap_or_default()
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
    #[serde(default)]
    pub is_admin: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeModeHealthRequest {
    pub count: u8,
    #[serde(default = "default_safe_prefix")]
    pub user_prefix: String,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeModeHealth {
    pub platform_ok: bool,
    pub is_admin: bool,
    pub password_ok: bool,
    pub users: Vec<SafeModeUserStatus>,
    pub missing_users: Vec<String>,
    pub ready: bool,
    pub warnings: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierPreset {
    pub id: String,
    pub label: String,
    pub description: String,
    pub count: u8,
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    pub use_tier_delays: bool,
    pub slot_tiers: Vec<String>,
    pub aliases: Vec<String>,
}

mod models;
mod platform;
mod policy;

use models::{
    AppInfo, BatchResult, LaunchOptions, LaunchResult, SafeModeHealth, SafeModeHealthRequest,
    SafeModePrepareRequest, SafeModePrepareResult, SafeModeUserStatus, TierPreset,
};
use policy::{clamp_count, tier_presets, LaunchPolicy};

#[tauri::command]
fn get_app_info(app_path: Option<String>) -> AppInfo {
    #[cfg(target_os = "macos")]
    {
        let _ = platform::cleanup_legacy_clones();
    }
    let resolved = platform::resolve_app_path(app_path.as_deref());
    let pids = platform::list_wecom_pids();
    AppInfo {
        platform: platform::current_platform().into(),
        resolved_path: resolved.map(|p| p.to_string_lossy().into_owned()),
        running_count: pids.len(),
        running_pids: pids,
        is_admin: platform::is_elevated(),
    }
}

#[tauri::command]
fn resolve_path(app_path: Option<String>) -> Option<String> {
    platform::resolve_app_path(app_path.as_deref()).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn launch_one(options: LaunchOptions) -> Result<LaunchResult, String> {
    let mut opts = options;
    opts.count = 1;
    let mut batch = launch_batch_inner(opts).await?;
    batch
        .results
        .pop()
        .ok_or_else(|| "未产生启动结果".to_string())
}

#[tauri::command]
async fn launch_batch(options: LaunchOptions) -> Result<BatchResult, String> {
    launch_batch_inner(options).await
}

async fn launch_batch_inner(options: LaunchOptions) -> Result<BatchResult, String> {
    if options.windows_safe_mode && platform::current_platform() != "windows" {
        return Err("安全模式仅支持 Windows".into());
    }
    if options.windows_safe_mode {
        let health = platform::check_safe_mode_health(
            options.count,
            &options.safe_mode_user_prefix,
            options.safe_mode_password.as_deref(),
        );
        if !health.ready {
            return Err(health.summary);
        }
    }

    let count = clamp_count(options.count);
    let app = platform::resolve_app_path(options.app_path.as_deref())
        .ok_or_else(|| "未找到企业微信，请手动指定路径".to_string())?;

    let mut policy = LaunchPolicy::new(options.min_delay_ms, options.max_delay_ms);
    let mut results = Vec::with_capacity(count as usize);

    for i in 1..=count {
        if options.use_tier_delays {
            policy.wait_for_tier(options.tier_for_index(i)).await;
        } else {
            policy.wait_before_next().await;
        }

        match platform::prepare_next_instance(&options) {
            Ok(msg) => {
                let tier = options.tier_for_index(i);
                let mut one = platform::spawn_instance(&app, i, &options)?;
                one.message = format!(
                    "{msg} | [{}] {}",
                    tier.label(),
                    one.message
                );
                let ok = one.success;
                results.push(one);
                if !ok {
                    break;
                }
            }
            Err(e) => {
                results.push(LaunchResult {
                    success: false,
                    pid: None,
                    message: e,
                    index: i,
                });
                break;
            }
        }
    }

    Ok(BatchResult {
        results,
        platform: platform::current_platform().into(),
    })
}

#[tauri::command]
fn list_running() -> Vec<u32> {
    platform::list_wecom_pids()
}

#[tauri::command]
fn kill_all() -> usize {
    platform::kill_all_wecom()
}

#[tauri::command]
fn prepare_safe_mode_users(req: SafeModePrepareRequest) -> Result<SafeModePrepareResult, String> {
    platform::ensure_safe_mode_users(req.count, &req.password, &req.user_prefix)
}

#[tauri::command]
fn list_safe_mode_users(count: u8, user_prefix: Option<String>) -> Vec<SafeModeUserStatus> {
    let prefix = user_prefix.unwrap_or_else(|| models::DEFAULT_SAFE_USER_PREFIX.to_string());
    platform::list_safe_mode_users(count, &prefix)
}

#[tauri::command]
fn check_safe_mode_health(req: SafeModeHealthRequest) -> SafeModeHealth {
    platform::check_safe_mode_health(req.count, &req.user_prefix, req.password.as_deref())
}

#[tauri::command]
fn get_tier_presets() -> Vec<TierPreset> {
    tier_presets()
}

#[tauri::command]
fn cleanup_macos_clones() -> usize {
    #[cfg(target_os = "macos")]
    {
        platform::cleanup_legacy_clones()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            resolve_path,
            launch_one,
            launch_batch,
            list_running,
            kill_all,
            prepare_safe_mode_users,
            list_safe_mode_users,
            check_safe_mode_health,
            get_tier_presets,
            cleanup_macos_clones
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

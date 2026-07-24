mod models;
mod platform;
mod policy;

use models::{
    AppInfo, BatchResult, LaunchOptions, LaunchResult, SafeModePrepareRequest,
    SafeModePrepareResult, SafeModeUserStatus,
};
use policy::{clamp_count, LaunchPolicy};

#[tauri::command]
fn get_app_info(app_path: Option<String>) -> AppInfo {
    let resolved = platform::resolve_app_path(app_path.as_deref());
    let pids = platform::list_wecom_pids();
    AppInfo {
        platform: platform::current_platform().into(),
        resolved_path: resolved.map(|p| p.to_string_lossy().into_owned()),
        running_count: pids.len(),
        running_pids: pids,
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
        let pw = options
            .safe_mode_password
            .as_deref()
            .map(str::trim)
            .unwrap_or("");
        if pw.is_empty() {
            return Err("已开启安全模式，请填写本地用户密码".into());
        }
    }

    let count = clamp_count(options.count);
    let app = platform::resolve_app_path(options.app_path.as_deref())
        .ok_or_else(|| "未找到企业微信，请手动指定路径".to_string())?;

    let mut policy = LaunchPolicy::new(options.min_delay_ms, options.max_delay_ms);
    let mut results = Vec::with_capacity(count as usize);

    for i in 1..=count {
        policy.wait_before_next().await;

        match platform::prepare_next_instance(&options) {
            Ok(msg) => {
                let mut one = platform::spawn_instance(&app, i, &options)?;
                one.message = format!("{msg} | {}", one.message);
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
            list_safe_mode_users
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

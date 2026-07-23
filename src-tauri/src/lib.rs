mod models;
mod platform;
mod policy;

use models::{AppInfo, BatchResult, LaunchOptions, LaunchResult};
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
    let count = clamp_count(options.count);
    let app = platform::resolve_app_path(options.app_path.as_deref())
        .ok_or_else(|| "未找到企业微信，请手动指定路径".to_string())?;

    let mut policy = LaunchPolicy::new(options.min_delay_ms, options.max_delay_ms);
    let mut results = Vec::with_capacity(count as usize);

    for i in 1..=count {
        policy.wait_before_next().await;

        match platform::prepare_next_instance(&options) {
            Ok(msg) => {
                // attach prepare info into next result message prefix
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
            kill_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

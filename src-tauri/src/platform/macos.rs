//! macOS backend: multi-open via `open -n` on the official app only.
//! Never clones .app into ~/Applications (avoids Launchpad clutter).

use crate::models::{LaunchOptions, LaunchResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn resolve_app_path(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    for candidate in [
        "/Applications/企业微信.app",
        "/Applications/WeCom.app",
        "/Applications/WXWork.app",
    ] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

pub fn prepare_next_instance(_opts: &LaunchOptions) -> Result<String, String> {
    // Best-effort cleanup of legacy clones from older launcher versions.
    let cleaned = cleanup_legacy_clones();
    if cleaned > 0 {
        Ok(format!("macOS：已清理 {cleaned} 个旧镜像，准备 open -n 启动"))
    } else {
        Ok("macOS：使用 open -n 启动官方企业微信（不创建镜像）".into())
    }
}

pub fn spawn_instance(
    app_path: &Path,
    index: u8,
    _opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    // Prefer launching the Mach-O binary directly; falls back to `open -n`.
    if let Ok(exe) = find_macos_executable(app_path) {
        match Command::new(&exe).spawn() {
            Ok(child) => {
                let pid = child.id();
                return Ok(LaunchResult {
                    success: true,
                    pid: Some(pid),
                    message: format!("已启动实例 #{index} PID={pid}（官方客户端，无镜像）"),
                    index,
                });
            }
            Err(e) => {
                // fall through to open -n
                let _ = e;
            }
        }
    }

    spawn_open_n(app_path, index)
}

fn spawn_open_n(app_path: &Path, index: u8) -> Result<LaunchResult, String> {
    let status = Command::new("open")
        .arg("-n")
        .arg("-a")
        .arg(app_path)
        .status()
        .map_err(|e| format!("open -n 失败: {e}"))?;

    if status.success() {
        Ok(LaunchResult {
            success: true,
            pid: None,
            message: format!("已通过 open -n 启动实例 #{index}（官方客户端，无镜像）"),
            index,
        })
    } else {
        Ok(LaunchResult {
            success: false,
            pid: None,
            message: format!("open -n 退出码: {:?}", status.code()),
            index,
        })
    }
}

/// Remove clones created by older versions under ~/Applications/WeComMulti.
pub fn cleanup_legacy_clones() -> usize {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let root = home.join("Applications").join("WeComMulti");
    if !root.exists() {
        return 0;
    }

    let mut n = 0usize;
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("app") {
                if fs::remove_dir_all(&path).is_ok() {
                    n += 1;
                }
            }
        }
    }
    // Remove empty folder if possible
    let _ = fs::remove_dir(&root);

    // Also clear old per-instance HOME stubs (optional data dirs)
    let data = home
        .join("Library")
        .join("Application Support")
        .join("WeComLauncher");
    if data.exists() {
        let _ = fs::remove_dir_all(&data);
    }

    n
}

fn find_macos_executable(app: &Path) -> Result<PathBuf, String> {
    let macos_dir = app.join("Contents/MacOS");
    let entries = fs::read_dir(&macos_dir).map_err(|e| format!("读取 MacOS 目录失败: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(format!("未在 {} 找到可执行文件", macos_dir.display()))
}

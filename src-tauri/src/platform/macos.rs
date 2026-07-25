//! macOS backend: multi-open via isolated .app copies with unique Bundle IDs.
//! Clones live under Application Support (NOT ~/Applications) so Launchpad stays clean.

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
    let cleaned = cleanup_legacy_launchpad_clones();
    if cleaned > 0 {
        Ok(format!(
            "macOS：已清理 {cleaned} 个启动台旧镜像；多开使用隐藏目录隔离实例"
        ))
    } else {
        Ok("macOS：使用隐藏隔离实例多开（不进入启动台）".into())
    }
}

pub fn spawn_instance(
    app_path: &Path,
    index: u8,
    _opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    // WeCom ignores plain open -n / same-binary relaunch. Need unique Bundle ID copies.
    let instance_app = ensure_hidden_clone(app_path, index)?;
    let exe = find_macos_executable(&instance_app)?;

    // Launch Mach-O directly (avoid `open` registering into Launchpad).
    match Command::new(&exe).spawn() {
        Ok(child) => {
            let pid = child.id();
            Ok(LaunchResult {
                success: true,
                pid: Some(pid),
                message: format!(
                    "已启动隔离实例 #{index} PID={pid}（隐藏目录，不进启动台）"
                ),
                index,
            })
        }
        Err(e) => {
            // Fallback: open -n on the hidden clone
            let status = Command::new("open")
                .arg("-n")
                .arg(&instance_app)
                .status()
                .map_err(|err| format!("启动失败: {e}; open 也失败: {err}"))?;
            if status.success() {
                Ok(LaunchResult {
                    success: true,
                    pid: None,
                    message: format!("已通过 open -n 启动隔离实例 #{index}"),
                    index,
                })
            } else {
                Ok(LaunchResult {
                    success: false,
                    pid: None,
                    message: format!("启动隔离实例失败，退出码 {:?}", status.code()),
                    index,
                })
            }
        }
    }
}

fn instances_root() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library")
        .join("Application Support")
        .join("WeComLauncher")
        .join("Instances")
}

fn instance_app_path(index: u8) -> PathBuf {
    // Neutral internal name — not shown in Launchpad when kept out of Applications.
    instances_root().join(format!("WeComInstance{index}.app"))
}

fn ensure_hidden_clone(source_app: &Path, index: u8) -> Result<PathBuf, String> {
    let root = instances_root();
    fs::create_dir_all(&root).map_err(|e| format!("创建隐藏实例目录失败: {e}"))?;

    let dest = instance_app_path(index);
    let need_rebuild = !dest.exists() || !bundle_looks_valid(&dest);

    if need_rebuild {
        if dest.exists() {
            let _ = fs::remove_dir_all(&dest);
        }
        create_app_clone(source_app, &dest, index)?;
    }

    // Unregister from Launch Services if it somehow got indexed.
    unregister_from_launch_services(&dest);
    Ok(dest)
}

fn bundle_looks_valid(app: &Path) -> bool {
    app.join("Contents/Info.plist").exists() && find_macos_executable(app).is_ok()
}

fn create_app_clone(source_app: &Path, dest: &Path, index: u8) -> Result<(), String> {
    let status = Command::new("cp")
        .arg("-R")
        .arg(source_app)
        .arg(dest)
        .status()
        .map_err(|e| format!("复制 .app 失败: {e}"))?;
    if !status.success() {
        return Err("复制 .app 失败".into());
    }

    let plist = dest.join("Contents/Info.plist");
    // Unique Bundle ID → independent sandbox container (required for real multi-open).
    let bundle_id = format!("com.wecomlauncher.instance{index}");
    let display = format!("WeComInst{index}");

    let _ = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Set :CFBundleIdentifier {bundle_id}"))
        .arg(&plist)
        .status();
    let _ = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Set :CFBundleName {display}"))
        .arg(&plist)
        .status();
    let _ = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Set :CFBundleDisplayName {display}"))
        .arg(&plist)
        .status();
    // Hide from Dock bounce spam a bit; still shows as windowed app when running.
    let _ = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg("Add :LSUIElement bool false")
        .arg(&plist)
        .status();

    let _ = Command::new("/usr/bin/xattr")
        .arg("-cr")
        .arg(dest)
        .status();

    let output = Command::new("codesign")
        .arg("--force")
        .arg("--deep")
        .arg("--sign")
        .arg("-")
        .arg("--timestamp=none")
        .arg(dest)
        .output()
        .map_err(|e| format!("codesign 失败: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        // Still try to run; some systems allow it after xattr clear.
        eprintln!("codesign warning: {err}");
    }

    unregister_from_launch_services(dest);
    Ok(())
}

fn unregister_from_launch_services(app: &Path) {
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = Command::new(lsregister)
        .arg("-u")
        .arg(app)
        .output();
}

/// Remove old clones that polluted Launchpad (~/Applications/WeComMulti).
pub fn cleanup_legacy_clones() -> usize {
    cleanup_legacy_launchpad_clones()
}

fn cleanup_legacy_launchpad_clones() -> usize {
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
                unregister_from_launch_services(&path);
                if fs::remove_dir_all(&path).is_ok() {
                    n += 1;
                }
            }
        }
    }
    let _ = fs::remove_dir(&root);
    n
}

fn find_macos_executable(app: &Path) -> Result<PathBuf, String> {
    let macos_dir = app.join("Contents/MacOS");
    let entries = fs::read_dir(&macos_dir).map_err(|e| format!("读取 MacOS 目录失败: {e}"))?;
    // Prefer non-helper binaries (largest / name match).
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    candidates.sort_by(|a, b| {
        let sa = a
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        let sb = b
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        sb.cmp(&sa)
    });
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| format!("未在 {} 找到可执行文件", macos_dir.display()))
}

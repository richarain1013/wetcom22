//! macOS backend: multi-open via isolated .app copies with unique Bundle IDs.
//! Clones live under Application Support (NOT ~/Applications) so Launchpad stays clean.
//!
//! Critical for WeCom CEF builds:
//! - Launch with `open -n` (direct Mach-O spawn kills GPU helpers).
//! - Re-sign with sandbox entitlements kept ON so each Bundle ID gets its own container.
//! - Never strip sandbox / never share Documents/cefcache across instances.

use crate::models::{LaunchOptions, LaunchResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Marker written next to a healthy clone so we can invalidate old broken copies.
const CLONE_FORMAT: &str = "2"; // bump when clone recipe changes

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
            "macOS：已清理 {cleaned} 个启动台旧镜像；使用沙盒隔离实例多开"
        ))
    } else {
        Ok("macOS：沙盒隔离实例多开（open -n，不进启动台）".into())
    }
}

pub fn spawn_instance(
    app_path: &Path,
    index: u8,
    _opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    let instance_app = ensure_hidden_clone(app_path, index)?;

    // Prefer Launch Services. Direct exec of WeCom's CEF binary dies with
    // "GPU process isn't usable" under ad-hoc copies.
    let status = Command::new("open")
        .arg("-n")
        .arg(&instance_app)
        .status()
        .map_err(|e| format!("open -n 启动失败: {e}"))?;

    if !status.success() {
        return Ok(LaunchResult {
            success: false,
            pid: None,
            message: format!(
                "open -n 失败，退出码 {:?}（可尝试：xattr -cr 实例目录后重试）",
                status.code()
            ),
            index,
        });
    }

    // Give CEF helpers time to spawn; confirm the instance actually stayed up.
    thread::sleep(Duration::from_millis(1800));
    let pids = pids_for_instance(&instance_app);
    if pids.is_empty() {
        // One retry — first launch after clone/codesign is sometimes slow.
        thread::sleep(Duration::from_millis(2200));
        let pids2 = pids_for_instance(&instance_app);
        if pids2.is_empty() {
            let _ = fs::remove_dir_all(&instance_app);
            let _ = fs::remove_file(clone_marker_path(index));
            return Ok(LaunchResult {
                success: false,
                pid: None,
                message: format!(
                    "实例 #{index} 启动后立即退出。已删除坏副本，请再点一次「新开 1 个」。"
                ),
                index,
            });
        }
        return Ok(LaunchResult {
            success: true,
            pid: pids2.first().copied(),
            message: format!(
                "已启动隔离实例 #{index} PID={}（沙盒容器，open -n）",
                pids2[0]
            ),
            index,
        });
    }

    Ok(LaunchResult {
        success: true,
        pid: pids.first().copied(),
        message: format!(
            "已启动隔离实例 #{index} PID={}（沙盒容器，open -n）",
            pids[0]
        ),
        index,
    })
}

fn instances_root() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library")
        .join("Application Support")
        .join("WeComLauncher")
        .join("Instances")
}

fn instance_app_path(index: u8) -> PathBuf {
    instances_root().join(format!("WeComInstance{index}.app"))
}

fn clone_marker_path(index: u8) -> PathBuf {
    instances_root().join(format!(".instance{index}.fmt"))
}

fn expected_bundle_id(index: u8) -> String {
    format!("com.tencent.WeWorkMac.instance{index}")
}

fn ensure_hidden_clone(source_app: &Path, index: u8) -> Result<PathBuf, String> {
    let root = instances_root();
    fs::create_dir_all(&root).map_err(|e| format!("创建隐藏实例目录失败: {e}"))?;

    let dest = instance_app_path(index);
    let need_rebuild = !dest.exists()
        || !bundle_looks_valid(&dest)
        || !clone_format_ok(index)
        || !bundle_id_matches(&dest, index);

    if need_rebuild {
        if dest.exists() {
            let _ = fs::remove_dir_all(&dest);
        }
        let _ = fs::remove_file(clone_marker_path(index));
        create_app_clone(source_app, &dest, index)?;
        fs::write(clone_marker_path(index), CLONE_FORMAT)
            .map_err(|e| format!("写入实例标记失败: {e}"))?;
    }

    Ok(dest)
}

fn clone_format_ok(index: u8) -> bool {
    fs::read_to_string(clone_marker_path(index))
        .map(|s| s.trim() == CLONE_FORMAT)
        .unwrap_or(false)
}

fn bundle_id_matches(app: &Path, index: u8) -> bool {
    let plist = app.join("Contents/Info.plist");
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier", &plist.to_string_lossy()])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let id = String::from_utf8_lossy(&o.stdout).trim().to_string();
            id == expected_bundle_id(index)
        }
        _ => false,
    }
}

fn bundle_looks_valid(app: &Path) -> bool {
    app.join("Contents/Info.plist").exists() && find_macos_executable(app).is_ok()
}

fn create_app_clone(source_app: &Path, dest: &Path, index: u8) -> Result<(), String> {
    // Prefer APFS clonefile (instant, CoW).
    let copied = Command::new("cp")
        .args(["-cR"])
        .arg(source_app)
        .arg(dest)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !copied {
        let status = Command::new("cp")
            .args(["-R"])
            .arg(source_app)
            .arg(dest)
            .status()
            .map_err(|e| format!("复制 .app 失败: {e}"))?;
        if !status.success() {
            return Err("复制 .app 失败".into());
        }
    }

    let plist = dest.join("Contents/Info.plist");
    let bundle_id = expected_bundle_id(index);
    let display = format!("WeComInst{index}");

    plist_set(&plist, "CFBundleIdentifier", &bundle_id)?;
    let _ = plist_set(&plist, "CFBundleName", &display);
    let _ = plist_set(&plist, "CFBundleDisplayName", &display);

    // Invalidate old seal before ad-hoc resign.
    let _ = fs::remove_dir_all(dest.join("Contents/_CodeSignature"));

    let _ = Command::new("/usr/bin/xattr")
        .args(["-cr"])
        .arg(dest)
        .status();

    let ents_path = write_clone_entitlements()?;
    let output = Command::new("codesign")
        .args([
            "--force",
            "--deep",
            "--sign",
            "-",
            "--entitlements",
        ])
        .arg(&ents_path)
        .arg("--timestamp=none")
        .arg(dest)
        .output()
        .map_err(|e| format!("codesign 失败: {e}"))?;

    let _ = fs::remove_file(&ents_path);

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("codesign 失败: {err}"));
    }

    Ok(())
}

fn plist_set(plist: &Path, key: &str, value: &str) -> Result<(), String> {
    let set = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Set :{key} {value}"))
        .arg(plist)
        .status()
        .map_err(|e| e.to_string())?;
    if set.success() {
        return Ok(());
    }
    let add = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Add :{key} string {value}"))
        .arg(plist)
        .status()
        .map_err(|e| e.to_string())?;
    if add.success() {
        Ok(())
    } else {
        Err(format!("无法写入 Info.plist 键 {key}"))
    }
}

/// Sandbox ON + CEF-friendly CS flags. No team app-groups / fixed mach ports
/// (those collide across instances under ad-hoc signing).
fn write_clone_entitlements() -> Result<PathBuf, String> {
    let path = instances_root().join(".clone-entitlements.plist");
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>com.apple.security.app-sandbox</key>
	<true/>
	<key>com.apple.security.cs.allow-jit</key>
	<true/>
	<key>com.apple.security.cs.allow-unsigned-executable-memory</key>
	<true/>
	<key>com.apple.security.cs.disable-library-validation</key>
	<true/>
	<key>com.apple.security.network.client</key>
	<true/>
	<key>com.apple.security.network.server</key>
	<true/>
	<key>com.apple.security.device.audio-input</key>
	<true/>
	<key>com.apple.security.device.camera</key>
	<true/>
	<key>com.apple.security.device.microphone</key>
	<true/>
	<key>com.apple.security.files.user-selected.read-write</key>
	<true/>
	<key>com.apple.security.files.downloads.read-write</key>
	<true/>
	<key>com.apple.security.files.bookmarks.app-scope</key>
	<true/>
	<key>com.apple.security.assets.pictures.read-write</key>
	<true/>
	<key>com.apple.security.print</key>
	<true/>
</dict>
</plist>
"#;
    fs::write(&path, xml).map_err(|e| format!("写入 entitlements 失败: {e}"))?;
    Ok(path)
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

fn unregister_from_launch_services(app: &Path) {
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = Command::new(lsregister).arg("-u").arg(app).output();
}

fn find_macos_executable(app: &Path) -> Result<PathBuf, String> {
    let macos_dir = app.join("Contents/MacOS");
    let exe_name = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            "Print :CFBundleExecutable",
            &app.join("Contents/Info.plist").to_string_lossy(),
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    if let Some(name) = exe_name {
        let p = macos_dir.join(&name);
        if p.is_file() {
            return Ok(p);
        }
    }

    let entries = fs::read_dir(&macos_dir).map_err(|e| format!("读取 MacOS 目录失败: {e}"))?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    candidates.sort_by(|a, b| {
        let sa = a.metadata().map(|m| m.len()).unwrap_or(0);
        let sb = b.metadata().map(|m| m.len()).unwrap_or(0);
        sb.cmp(&sa)
    });
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| format!("未在 {} 找到可执行文件", macos_dir.display()))
}

fn pids_for_instance(instance_app: &Path) -> Vec<u32> {
    let needle = instance_app.join("Contents/MacOS").to_string_lossy().to_string();
    let output = Command::new("pgrep").args(["-f", &needle]).output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
            .collect(),
        _ => Vec::new(),
    }
}

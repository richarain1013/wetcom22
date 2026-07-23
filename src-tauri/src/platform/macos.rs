//! macOS backend: clone .app with unique Bundle ID + isolated HOME,
//! or lightweight `open -n` on the original app.
//! Zero injection into WeCom.

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
    // macOS does not use the Windows ExclusiveObject mutex path.
    Ok("macOS：准备启动新实例".into())
}

pub fn spawn_instance(
    app_path: &Path,
    index: u8,
    opts: &LaunchOptions,
) -> Result<LaunchResult, String> {
    if opts.macos_clone_instances {
        spawn_cloned(app_path, index)
    } else {
        spawn_open_n(app_path, index)
    }
}

fn spawn_open_n(app_path: &Path, index: u8) -> Result<LaunchResult, String> {
    let status = Command::new("open")
        .arg("-n")
        .arg(app_path)
        .status()
        .map_err(|e| format!("open -n 失败: {e}"))?;

    if status.success() {
        Ok(LaunchResult {
            success: true,
            pid: None, // `open` returns quickly; PID belongs to LaunchServices
            message: format!("已通过 open -n 启动实例 #{index}"),
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

fn instances_root() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Applications").join("WeComMulti")
}

fn instance_data_root(index: u8) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("Library")
        .join("Application Support")
        .join("WeComLauncher")
        .join(format!("instance-{index}"))
}

fn spawn_cloned(source_app: &Path, index: u8) -> Result<LaunchResult, String> {
    let instance_app = create_app_clone(source_app, index)?;
    let data_home = instance_data_root(index);
    fs::create_dir_all(&data_home).map_err(|e| format!("创建数据目录失败: {e}"))?;
    fs::create_dir_all(data_home.join("tmp")).ok();
    fs::create_dir_all(data_home.join("Documents")).ok();

    let exe = find_macos_executable(&instance_app)?;

    let child = Command::new(&exe)
        .env("HOME", &data_home)
        .env("TMPDIR", data_home.join("tmp"))
        .spawn()
        .or_else(|_| {
            Command::new("open")
                .arg("-n")
                .arg(&instance_app)
                .spawn()
        })
        .map_err(|e| format!("启动克隆实例失败: {e}"))?;

    let pid = child.id();
    Ok(LaunchResult {
        success: true,
        pid: Some(pid),
        message: format!(
            "已启动克隆实例 #{index} PID={pid} ({})",
            instance_app.display()
        ),
        index,
    })
}

fn create_app_clone(source_app: &Path, index: u8) -> Result<PathBuf, String> {
    let root = instances_root();
    fs::create_dir_all(&root).map_err(|e| format!("创建实例目录失败: {e}"))?;

    let stem = source_app
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("WeCom");
    let dest = root.join(format!("{stem}-{index}.app"));

    if dest.exists() {
        // Reuse existing clone for faster subsequent launches
        return Ok(dest);
    }

    let status = Command::new("cp")
        .arg("-R")
        .arg(source_app)
        .arg(&dest)
        .status()
        .map_err(|e| format!("复制 .app 失败: {e}"))?;
    if !status.success() {
        return Err("复制 .app 失败".into());
    }

    let plist = dest.join("Contents/Info.plist");
    let bundle_id = format!("com.tencent.WeWorkMac.launcher{index}");
    let _ = Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Set :CFBundleIdentifier {bundle_id}"))
        .arg(&plist)
        .status();

    let _ = Command::new("/usr/bin/xattr")
        .arg("-rc")
        .arg(&dest)
        .status();

    let _ = Command::new("codesign")
        .arg("--force")
        .arg("--deep")
        .arg("--sign")
        .arg("-")
        .arg("--timestamp=none")
        .arg(&dest)
        .output();

    Ok(dest)
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

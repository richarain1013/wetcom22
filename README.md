# WeCom Multi Launcher

跨平台（**Windows + macOS**）企业微信多账号启动器。一套代码：Tauri 2 + Rust + TypeScript。

## 快速入口

- **macOS**：见下方「开发环境」；已打包的 `.app` 在 `dist-app/` 或「应用程序」
- **Windows**：用 GitHub Actions 打安装包（推荐），见 **[docs/WINDOWS.md](docs/WINDOWS.md)**  
  - Actions → **Release** → Run workflow，或 `git tag v0.1.0 && git push --tags`  
  - 在 Releases 下载 `.msi` / `setup.exe` 拷到 Windows 安装  
- **安全模式 / 分档 / 虚拟机**：见 [SAFE_MODE.md](docs/SAFE_MODE.md)、[VM_GUIDE.md](docs/VM_GUIDE.md)

## 平台差异

| | Windows | macOS |
|--|---------|-------|
| 多开手段 | 注册表 `multi_instances` + 进程外释放 Mutex | 克隆 `.app`（改 Bundle ID）或 `open -n` |
| 默认路径 | `WXWork.exe` | `/Applications/企业微信.app` |
| 8–10 开建议 | 分批 + 2.5–6s 抖动 | 开启「克隆 .app」 |

两端均：**零注入、不改官方二进制内容（macOS 仅复制一份 .app 改 Bundle ID）**。

## 开发环境

- Node.js 18+
- Rust stable（本仓库可用本地 `.cargo` / `.rustup`）
- macOS：Xcode CLT；Windows：MSVC Build Tools + WebView2

### macOS

```bash
cd wecom-multi-launcher

# 若使用项目内 Rust：
export RUSTUP_HOME="$PWD/.rustup"
export CARGO_HOME="$PWD/.cargo"
source "$CARGO_HOME/env"

npm install
npm run tauri:dev
```

打包：`npm run tauri:build` → `.dmg` / `.app`

### Windows

```powershell
cd wecom-multi-launcher
npm install
npm run tauri:build
# 或: .\scripts\build-windows.ps1
```

产物：`src-tauri\target\release\bundle\` 下的 `.msi` / `setup.exe`  
详情：[docs/WINDOWS.md](docs/WINDOWS.md)

## 目录

```
wecom-multi-launcher/
├── src/                 # 前端 UI（两端共用）
├── src-tauri/
│   └── src/
│       ├── lib.rs       # Tauri 命令
│       ├── models.rs
│       ├── policy.rs    # 分批抖动
│       └── platform/
│           ├── windows.rs
│           └── macos.rs
├── docs/
│   └── WINDOWS.md       # Windows 安装使用说明
└── legacy-wpf/          # 旧版仅 Windows 的 C# 骨架（已归档）
```

## 风险说明

非官方能力，存在协议与风控风险。详见 [docs/ANTI_DETECTION.md](docs/ANTI_DETECTION.md)。

# 企业微信多账号启动器（骨架）

Windows 桌面启动器：在同一台电脑上管理 **8–10 个**企业微信官方客户端实例。

技术栈：**C# / .NET 8 WPF**（本机未装 Rust，故未用 Tauri；Windows 进程/句柄管理用 C# 更直接）。

## 设计原则（低特征 / 规避风控的正确姿势）

本项目刻意 **不做** 外挂式对抗，而是用「看起来像正常人多次打开官方客户端」的方式降低风险：

| 做 | 不做 |
|----|------|
| 只启动官方 `WXWork.exe` | 不注入 DLL / 不远程线程 |
| 进程外关闭单实例 Mutex | 不改企微内存 / 不 Patch 二进制 |
| 注册表探测 `multi_instances`（若版本仍支持） | 不 Hook 网络 / 不改设备指纹 |
| 分批启动 + 随机间隔（2.5–6s） | 不自动扫码、不群控 UI 自动化 |
| `UseShellExecute=true` 正常拉起 | 不驱动级隐藏进程 |

> 服务端仍可能通过同一设备指纹关联多账号。启动器无法「对抗」服务端策略；能做的是避免客户端侧注入特征。详见 [docs/ANTI_DETECTION.md](docs/ANTI_DETECTION.md)。

## 目录结构

```
wecom-multi-launcher/
├── WeComLauncher.sln
├── docs/
│   ├── ARCHITECTURE.md
│   └── ANTI_DETECTION.md
└── src/WeComLauncher/
    ├── Services/          # 路径解析、注册表、Mutex 释放、分批策略、实例管理
    ├── Native/            # 最小 P/Invoke 面
    ├── Models/
    ├── ViewModels/
    └── MainWindow.xaml
```

## 环境要求

- Windows 10/11 x64
- [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0)
- 已安装企业微信 PC 版
- 关闭互斥句柄失败时，可能需要 **以管理员运行** 一次

## 构建与运行

在 **Windows** 上：

```powershell
cd wecom-multi-launcher
dotnet restore
dotnet build -c Release
dotnet run --project src\WeComLauncher -c Release
```

发布单文件：

```powershell
dotnet publish src\WeComLauncher -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true -o .\dist
```

## 推荐使用方式（8–10 账号）

1. 先完全退出企业微信  
2. 打开本启动器，确认已定位到 `WXWork.exe`  
3. 批量数量设为 `8` 或 `10`，间隔保持默认 `2500–6000ms`  
4. 点「分批启动」，每个窗口扫码登录不同账号  
5. 不要在短时间内反复杀进程再秒开全部账号  

## 配置存放

`%AppData%\WeComLauncher\settings.json`

## 风险声明

- 非官方能力，可能违反企业微信用户协议  
- 企微升级后 Mutex 名称可能变化，需更新 `MutexReleaseService` 中的匹配列表  
- 多账号同设备存在封号 / 风控风险，请自行评估  
- 仅供学习与内部运维场景；请勿用于违规营销或骚扰  

## 后续可扩展

- 系统托盘常驻  
- 每个槽位绑定「上次登录企业名」备忘（不读企微数据库）  
- Safe Mode：用 Windows 本地用户隔离启动（最稳、最慢）  
- 企微版本指纹检测 + 兼容矩阵  

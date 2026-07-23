# 跨平台架构

```
┌─────────────────────────────────────┐
│  Frontend (Vite + TypeScript)       │  ← 同一套 UI
│  invoke(launch_batch / launch_one)  │
└─────────────────┬───────────────────┘
                  │ Tauri IPC
┌─────────────────▼───────────────────┐
│  lib.rs  编排 + LaunchPolicy        │
└─────────────────┬───────────────────┘
        ┌─────────┴─────────┐
        ▼                   ▼
┌───────────────┐   ┌───────────────┐
│ windows.rs    │   │ macos.rs      │
│ Mutex 释放    │   │ 克隆 .app /   │
│ 注册表探测    │   │ open -n       │
│ CreateProcess │   │ 独立 HOME     │
└───────────────┘   └───────────────┘
```

## 公共契约

两端实现同一组函数：

- `resolve_app_path`
- `prepare_next_instance`
- `spawn_instance`
- `list_wecom_pids` / `kill_all_wecom`（`common.rs` + sysinfo）

## 为何用 Tauri

- 一套前端 + 条件编译后端，真正「一份代码两边跑」
- 体积小于 Electron
- Windows Mutex / macOS `open`/`cp` 都能用系统 API 干净实现

# 低特征策略（Windows + macOS）

## 共同原则

1. **零注入**：不向企业微信进程注入 DLL/dylib  
2. **不改官方安装包本体**：Windows 只启动原 `WXWork.exe`；macOS 多开时复制一份 `.app` 并改 Bundle ID（不改 `/Applications` 原件）  
3. **分批 + 抖动**：8–10 开默认 2.5–6 秒随机间隔  
4. **不做**：指纹伪造、驱动隐藏、消息 Hook、自动群发  

## Windows

- 优先软写 `HKCU\...\multi_instances`  
- 再进程外 `DuplicateHandle(DUPLICATE_CLOSE_SOURCE)` 释放 ExclusiveObject  
- 失败时提示提权，默认不以管理员常驻  

## macOS

- **克隆模式（默认，适合 8–10）**：`~/Applications/WeComMulti/` 下独立 `.app` + 独立 `HOME`  
- **轻量模式**：`open -n` 原应用（实现简单，数据隔离较弱）  
- 不依赖 Windows 那套 Mutex  

## 预期

| 目标 | 能否保证 |
|------|----------|
| 同机多窗口登录 | 多数版本可以 |
| 客户端不报外挂 | 大概率（无注入） |
| 服务端不关联同设备 | **不能保证** |

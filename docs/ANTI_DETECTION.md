# 低特征策略（Windows + macOS）

## 共同原则

1. **零注入**：不向企业微信进程注入 DLL/dylib  
2. **不改官方安装包本体**：Windows 只启动原 `WXWork.exe`；macOS 多开时复制一份 `.app` 并改 Bundle ID（不改 `/Applications` 原件）  
3. **分批 + 抖动**：8–10 开默认 2.5–6 秒随机间隔  
4. **不做**：指纹伪造、驱动隐藏、消息 Hook、自动群发  

## Windows

- 优先软写 `HKCU\...\multi_instances`  
- 再进程外释放 ExclusiveObject（扩展名称匹配）  
- 用 `cmd start` / 独立控制台拉起，更接近双击启动  
- 每次启动前短暂等待，降低第二实例被吸回的概率  
- **安全模式**：每槽位本地用户，见 [SAFE_MODE.md](./SAFE_MODE.md)  

## macOS

- 在 `~/Library/Application Support/WeComLauncher/Instances/` 创建隔离副本（改 Bundle ID + 重签）  
- **不写入** `~/Applications`，避免启动台镜像  
- 直接启动 Mach-O，并用 `lsregister -u` 取消索引  
- 安装说明见 [MACOS.md](./MACOS.md)  

## 预期

| 目标 | 能否保证 |
|------|----------|
| 同机多窗口登录 | 多数版本可以 |
| 客户端配置隔离（安全模式） | 可以（分用户 Profile） |
| 客户端不报外挂 | 大概率（无注入） |
| 服务端不关联同设备 | **不能保证** |

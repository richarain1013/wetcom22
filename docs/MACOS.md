# macOS 安装与使用

## 安装（GitHub Release 的 .dmg）

Release 包为 **未公证** 的本地构建，macOS 会拦截。按下面做：

1. 打开 DMG，把 **WeCom Launcher** 拖到「应用程序」  
2. 若提示「无法打开 / 已损坏」：  
   - 打开「系统设置 → 隐私与安全性」→ 仍要打开  
   - 或在终端执行：
   ```bash
   xattr -cr "/Applications/WeCom Launcher.app"
   open "/Applications/WeCom Launcher.app"
   ```
3. 不要用「克隆企业微信.app」的旧方式；本版本只用官方客户端多开

## 多开原理（当前版本）

- 直接启动 `/Applications/企业微信.app` 内可执行文件，或 `open -n`  
- **不会**在启动台创建「企业微信-1…10」镜像  
- 若以前装过旧版，启动器会自动清理 `~/Applications/WeComMulti/`

## 使用

1. 确认已安装官方「企业微信」  
2. 打开 WeCom Launcher → 路径应自动识别  
3. 点「分批启动」或「新开 1 个」  
4. 每个窗口分别扫码登录  

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
3. 多开副本在隐藏目录，**不会**在启动台出现「企业微信-1」等镜像

## 多开原理（当前版本）

企微在 macOS 上会拦截「同一 Bundle 多开」，因此需要：

1. 在**隐藏目录**复制官方 `.app`：`~/Library/Application Support/WeComLauncher/Instances/`  
2. 修改独立 `CFBundleIdentifier` 并重新签名  
3. **直接启动可执行文件**（不放进 `~/Applications` / 启动台）

旧版曾写入 `~/Applications/WeComMulti/`，启动器会自动清理。

## 使用

1. 确认已安装官方「企业微信」  
2. 打开 WeCom Launcher → 路径应自动识别  
3. 点「新开 1 个」两次，应出现两个独立登录窗口  
4. 首次多开会复制应用，稍等片刻  
5. 每个窗口分别扫码登录  

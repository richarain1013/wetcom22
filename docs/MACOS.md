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

企微 Mac 版基于 CEF。对整包做 `codesign --deep` 会把 Helper 的腾讯 Developer ID 换成 ad-hoc，扫码页出现后约 15–25 秒 Helper 会 SIGTRAP，主进程随之退出。正确做法：

1. 在**隐藏目录**复制官方 `.app`：`~/Library/Application Support/WeComLauncher/Instances/`  
2. 修改独立 `CFBundleIdentifier`（如 `com.tencent.WeWorkMac.instanceN`）  
3. **只浅签名**外壳 + 主程序（保留 Helper 原签名），并加上 `disable-library-validation`  
4. 保留 App Sandbox → 每实例独立容器  
5. 用 **`open -n`** 启动  

旧坏副本（deep 重签）会按格式标记自动重建。

## 使用

1. 确认已安装官方「企业微信」  
2. 打开 WeCom Launcher（0.1.5+）→ 路径应自动识别  
3. 点「新开 1 个」——会自动占用下一个空闲槽位（#1、#2、#3…），**不要**重复打同一槽位  
4. 首次使用某槽位会复制+浅签名，约 10–30 秒；扫码窗应能一直保持  
5. 「分批启动」也只启动当前空闲槽位  
6. 若仍异常：完全退出企微，删除 `~/Library/Application Support/WeComLauncher/Instances/` 后再试 

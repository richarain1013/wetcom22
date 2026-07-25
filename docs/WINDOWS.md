# Windows 安装与使用

Mac 打出来的 `.app` **不能**在 Windows 上用。推荐用 **GitHub Actions** 自动打 Windows 安装包，下载后拷到 Windows 安装。

## 推荐：用 CI 下载安装包（无需本机 Windows 编译环境）

1. 把 `wecom-multi-launcher` 推到 GitHub 仓库（本目录作为仓库根目录）
2. 打开仓库 → **Actions** → **Release** → **Run workflow**  
   或打标签推送：`git tag v0.1.0 && git push origin v0.1.0`
3. 等待 Windows / macOS 构建完成
4. 打开 **Releases**，下载：
   - Windows：`*.msi` 或 `*-setup.exe`
   - macOS：`*.dmg`（可选）
5. 把安装包拷到 Windows 电脑，双击安装即可

详细说明见 [CI.md](./CI.md)。

## 一、本机环境准备（仅在你要自己打包时）

1. 安装 [Node.js 18+](https://nodejs.org/)（勾选加入 PATH）
2. 安装 [Rust](https://rustup.rs/)：打开页面下载 `rustup-init.exe`，一路默认
3. 安装 **Visual Studio Build Tools**（C++ 桌面开发）  
   - 下载：https://visualstudio.microsoft.com/visual-cpp-build-tools/  
   - 勾选「使用 C++ 的桌面开发」
4. 安装/确认已有 [WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/)（Win10/11 通常自带）
5. 已安装 **企业微信 PC 版**

装完后**新开**一个 PowerShell，检查：

```powershell
node -v
npm -v
rustc -V
cargo -V
```

## 二、本机打包（可选）

把整个 `wecom-multi-launcher` 文件夹拷到 Windows。

```powershell
cd 路径\wecom-multi-launcher
npm install
npm run tauri:build
# 或: .\scripts\build-windows.ps1
```

构建成功后，安装包一般在：

```
src-tauri\target\release\bundle\msi\WeCom Launcher_0.1.0_x64_en-US.msi
src-tauri\target\release\bundle\nsis\WeCom Launcher_0.1.0_x64-setup.exe
```

也可直接运行免安装版：

```
src-tauri\target\release\wecom-multi-launcher.exe
```

## 三、安装

- 双击 `.msi` 或 `*-setup.exe` 按提示安装  
- 开始菜单会出现 **WeCom Launcher**

首次被 SmartScreen 拦截时：更多信息 → 仍要运行（本地/CI 构建未签名属正常）。

## 四、使用（8–10 账号）

### 普通多开（先试这个）

1. **右键以管理员身份运行** WeCom Launcher（关互斥体必需）  
2. 先在托盘**完全退出**企业微信（不要只关窗口）  
3. 确认路径指向 `WXWork.exe`  
4. 不要勾选安全模式（先验证基础多开）  
5. 点「新开 1 个」——首次应在几秒内弹出登录窗（不要干等几分钟）  
6. 登录窗稳定后再点第二次做多开  
7. 若提示「句柄枚举超时」仍可能已启动成功，看桌面是否出现企微窗口  

### 安全模式（本地用户隔离）

详见 [SAFE_MODE.md](./SAFE_MODE.md)。

### 仍无法多开时

- 企微大版本可能改了互斥体名称，把日志发开发者适配  
- 杀软可能拦截关句柄，加入白名单  
- 部分公司策略禁止多开，与工具无关  
- **不要**在启动器卡住时反复连点；先结束任务管理器里的启动器进程再重开  

## 五、开发调试（可选）

```powershell
cd 路径\wecom-multi-launcher
npm install
npm run tauri:dev
```

## 常见问题

| 问题 | 处理 |
|------|------|
| `link.exe` / MSVC 报错 | 安装 VS Build Tools，并勾选 C++ 桌面开发 |
| 找不到 WebView2 | 安装 WebView2 Runtime |
| 只能开 1 个企微 | 管理员运行启动器；或企微升级后 Mutex 名变了需适配 |
| 杀软拦截 | 加入白名单（关句柄行为可能被误报） |
| CI 构建失败 | 看 Actions 日志；确认 `package-lock.json` 已提交 |

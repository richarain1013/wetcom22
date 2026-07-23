# CI / 发布说明

## 工作流

| 文件 | 触发 | 作用 |
|------|------|------|
| `.github/workflows/ci.yml` | push/PR 到 main/master | Windows + macOS 编译检查 |
| `.github/workflows/release.yml` | `v*` 标签，或手动 Run workflow | 打安装包并上传到 GitHub Release |

## 发布 Windows 安装包（推荐流程）

1. 将 **本目录 `wecom-multi-launcher` 作为仓库根目录** 推到 GitHub  
   （不要把上层小程序项目混在同一仓库根，否则 Actions 找不到 `src-tauri`）
2. 在 GitHub 网页：
   - **Actions** → **Release** → **Run workflow** → Run  
   - 或本地：`git tag v0.1.0 && git push origin v0.1.0`
3. 构建约需 10–20 分钟，完成后打开 **Releases**
4. 下载 Windows 的 `.msi` 或 `*-setup.exe`，拷到 Windows 电脑安装

手动触发（无 tag）会生成 **draft** Release，标题类似 `ci-<run_id>`，可在 Releases 草稿中下载资源。

## 产物

- Windows：NSIS installer / MSI
- macOS：DMG（arm64 + x64 各一份）

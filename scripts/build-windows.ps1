# Build WeCom Launcher for Windows
# Usage:  .\scripts\build-windows.ps1

$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

Write-Host "==> Checking toolchain..." -ForegroundColor Cyan
node -v | Out-Null
npm -v | Out-Null
rustc -V | Out-Null
cargo -V | Out-Null

Write-Host "==> npm install..." -ForegroundColor Cyan
npm install

Write-Host "==> tauri build..." -ForegroundColor Cyan
npm run tauri:build

$bundle = Join-Path $PWD "src-tauri\target\release\bundle"
Write-Host ""
Write-Host "Build finished. Look for installers under:" -ForegroundColor Green
Write-Host "  $bundle\msi"
Write-Host "  $bundle\nsis"
Write-Host "Or run:" -ForegroundColor Green
Write-Host "  src-tauri\target\release\wecom-multi-launcher.exe"

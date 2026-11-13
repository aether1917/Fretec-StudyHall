# 构建 Windows NSIS 安装包并输出到 out/
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

& (Join-Path $PSScriptRoot 'seed-nsis.ps1')

Write-Host '== 构建前端 + Tauri NSIS 安装包 =='
& pnpm tauri build --bundles nsis
if ($LASTEXITCODE -ne 0) { throw 'Tauri 打包失败' }

$outDir = Join-Path $root 'out'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$bundleDir = Join-Path $root 'src-tauri\target\release\bundle\nsis'
$installers = Get-ChildItem $bundleDir -Filter '*.exe'
if (-not $installers) { throw "未找到安装包：$bundleDir" }
foreach ($installer in $installers) {
    Copy-Item $installer.FullName (Join-Path $outDir $installer.Name) -Force
    Write-Host "已输出 out/$($installer.Name)"
}

# 预置 Tauri NSIS 打包工具链（网络受限时通过 gh-proxy 拉取）
$ErrorActionPreference = 'Stop'

$cacheRoot = Join-Path $env:LOCALAPPDATA 'tauri'
$nsisDir = Join-Path $cacheRoot 'NSIS'
$requiredDll = Join-Path $nsisDir 'Plugins\x86-unicode\additional\nsis_tauri_utils.dll'

if ((Test-Path (Join-Path $nsisDir 'makensis.exe')) -and (Test-Path $requiredDll)) {
    Write-Host 'NSIS 工具链已就绪，跳过下载'
    exit 0
}

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
$tmp = Join-Path $env:TEMP 'fretec-nsis-seed'
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

$zip = Join-Path $tmp 'nsis-3.13.zip'
$dll = Join-Path $tmp 'nsis_tauri_utils.dll'

function Get-Remote([string]$url, [string]$dest) {
    curl.exe -sL --fail --retry 3 --max-time 300 $url -o $dest
    if ($LASTEXITCODE -ne 0) { throw "下载失败：$url" }
}

Get-Remote 'https://gh-proxy.com/https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip' $zip
$zipHash = (Get-FileHash $zip -Algorithm SHA1).Hash
if ($zipHash -ne 'DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E') { throw "NSIS 压缩包校验失败：$zipHash" }

Get-Remote 'https://gh-proxy.com/https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll' $dll
$dllHash = (Get-FileHash $dll -Algorithm SHA1).Hash
if ($dllHash -ne '75197FEE3C6A814FE035788D1C34EAD39349B860') { throw "NSIS 插件校验失败：$dllHash" }

if (Test-Path $nsisDir) { Remove-Item -Recurse -Force $nsisDir }
Expand-Archive -Path $zip -DestinationPath $tmp -Force
Move-Item (Join-Path $tmp 'nsis-3.13') $nsisDir

$pluginDir = Join-Path $nsisDir 'Plugins\x86-unicode\additional'
New-Item -ItemType Directory -Force -Path $pluginDir | Out-Null
Copy-Item $dll (Join-Path $pluginDir 'nsis_tauri_utils.dll') -Force

Write-Host "NSIS 工具链已就绪：$nsisDir"

# 构建 Android APK 并输出到 out/
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $env:JAVA_HOME) {
    $jdk = Get-ChildItem 'C:\Program Files\Eclipse Adoptium' -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like 'jdk-17*' } | Select-Object -First 1
    if ($jdk) { $env:JAVA_HOME = $jdk.FullName }
}
if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
}
$env:NDK_HOME = if ($env:NDK_HOME) { $env:NDK_HOME } else {
    $ndk = Get-ChildItem (Join-Path $env:ANDROID_HOME 'ndk') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($ndk) { $ndk.FullName } else { '' }
}

if (-not $env:JAVA_HOME -or -not (Test-Path $env:ANDROID_HOME) -or -not $env:NDK_HOME) {
    throw '缺少 JDK / Android SDK / NDK，请先运行 scripts/setup-android.ps1'
}

Write-Host '== 构建 Android APK =='
& pnpm tauri android build --apk
if ($LASTEXITCODE -ne 0) { throw 'Android 打包失败' }

$outDir = Join-Path $root 'out'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$apkDir = Join-Path $root 'src-tauri\gen\android\app\build\outputs\apk'
$apks = Get-ChildItem $apkDir -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue
if (-not $apks) { throw "未找到 APK：$apkDir" }
foreach ($apk in $apks) {
    $name = "Fretec-StudyHall-0.1.0-$($apk.BaseName).apk"
    Copy-Item $apk.FullName (Join-Path $outDir $name) -Force
    Write-Host "已输出 out/$name"
}

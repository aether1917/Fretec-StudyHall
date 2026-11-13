# Build the Android APK into out/
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

. (Join-Path $PSScriptRoot 'dev-env.ps1')

if (-not $env:JAVA_HOME) {
    $jdk = Get-ChildItem 'C:\Program Files\Eclipse Adoptium' -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like 'jdk-17*' } | Select-Object -First 1
    if ($jdk) { $env:JAVA_HOME = $jdk.FullName }
}
if (-not $env:JAVA_HOME) {
    $localJdk = Join-Path $env:USERPROFILE '.tools\jdk17'
    $java = Get-ChildItem $localJdk -Recurse -Filter 'java.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like '*\bin\java.exe' } | Select-Object -First 1
    if ($java) { $env:JAVA_HOME = Split-Path -Parent (Split-Path -Parent $java.FullName) }
}
if (-not $env:ANDROID_HOME) {
    $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
}
if (-not $env:NDK_HOME) {
    $ndk = Get-ChildItem (Join-Path $env:ANDROID_HOME 'ndk') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($ndk) { $env:NDK_HOME = $ndk.FullName }
}

if (-not $env:JAVA_HOME -or -not (Test-Path $env:ANDROID_HOME) -or -not $env:NDK_HOME) {
    throw 'missing JDK / Android SDK / NDK, run scripts/setup-android.ps1 first'
}

Write-Host '== build Android APK =='
& pnpm tauri android build --apk
if ($LASTEXITCODE -ne 0) { throw 'android build failed' }

$outDir = Join-Path $root 'out'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$apkDir = Join-Path $root 'src-tauri\gen\android\app\build\outputs\apk'
$apks = Get-ChildItem $apkDir -Recurse -Filter '*.apk' -ErrorAction SilentlyContinue
if (-not $apks) { throw "apk not found: $apkDir" }
foreach ($apk in $apks) {
    $name = "Fretec-StudyHall-0.1.0-$($apk.BaseName).apk"
    Copy-Item $apk.FullName (Join-Path $outDir $name) -Force
    Write-Host "output out/$name"
}

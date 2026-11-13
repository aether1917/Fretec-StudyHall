# Install the Android build environment (JDK 17 + Android SDK + NDK + Rust targets).
# A JDK 17 is required first: winget install EclipseAdoptium.Temurin.17.JDK
$ErrorActionPreference = 'Stop'

$sdkRoot = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
$toolsDir = Join-Path $sdkRoot 'cmdline-tools'
$latest = Join-Path $toolsDir 'latest'
$sdkManager = Join-Path $latest 'bin\sdkmanager.bat'

if (-not (Test-Path $sdkManager)) {
    New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null
    $zip = Join-Path $env:TEMP 'commandlinetools-win.zip'
    Write-Host 'downloading Android command-line tools...'
    curl.exe -sL --fail --max-time 600 'https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip' -o $zip
    if ($LASTEXITCODE -ne 0) { throw 'download failed' }
    Expand-Archive -Path $zip -DestinationPath $toolsDir -Force
    $extracted = Join-Path $toolsDir 'cmdline-tools'
    if (Test-Path $extracted) { Move-Item $extracted $latest }
}

if (-not $env:JAVA_HOME) {
    $jdk = Get-ChildItem 'C:\Program Files\Eclipse Adoptium' -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like 'jdk-17*' } | Select-Object -First 1
    if ($jdk) { $env:JAVA_HOME = $jdk.FullName }
}
if (-not $env:JAVA_HOME) { throw 'JDK 17 not found, install it and set JAVA_HOME first' }

$env:ANDROID_HOME = $sdkRoot
$env:ANDROID_SDK_ROOT = $sdkRoot

Write-Host 'installing SDK packages...'
$packages = @(
    'platform-tools',
    'platforms;android-35',
    'build-tools;35.0.0',
    'ndk;27.2.12479018'
)
& $sdkManager --sdk_root=$sdkRoot @packages
if ($LASTEXITCODE -ne 0) { throw 'sdkmanager failed' }

Write-Host 'accepting SDK licenses...'
'y' * 50 | & $sdkManager --sdk_root=$sdkRoot --licenses

$env:NDK_HOME = Join-Path $sdkRoot 'ndk\27.2.12479018'

Write-Host 'adding Rust Android targets...'
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android

Write-Host ''
Write-Host 'Android environment ready. Set these variables and run scripts/package-android.ps1:'
Write-Host "  JAVA_HOME=$env:JAVA_HOME"
Write-Host "  ANDROID_HOME=$env:ANDROID_HOME"
Write-Host "  NDK_HOME=$env:NDK_HOME"

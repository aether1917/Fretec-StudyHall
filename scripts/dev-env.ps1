# Configure the local build environment (idempotent, ASCII-only).
# Standard machines with Visual Studio Build Tools do not need the xwin fallback;
# this script only wires it when the downloaded CRT/SDK and rc.exe are present.
$ErrorActionPreference = 'Continue'

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargoBin) {
    if (($env:Path -split ';') -notcontains $cargoBin) {
        $env:Path = "$cargoBin;$env:Path"
    }
}

$rcDir = Join-Path $env:USERPROFILE '.tools\sdk-buildtools\bin\10.0.22621.0\x64'
$xwinRoot = Join-Path $env:USERPROFILE '.xwin'
$lldLink = Join-Path $env:USERPROFILE '.tools\lld-link.exe'

if ((Test-Path (Join-Path $rcDir 'rc.exe')) -and (Test-Path $lldLink) -and (Test-Path (Join-Path $xwinRoot 'crt\lib\x86_64'))) {
    if (($env:Path -split ';') -notcontains $rcDir) {
        $env:Path = "$rcDir;$env:Path"
    }
    $env:RC = Join-Path $rcDir 'rc.exe'
    $env:RC_x86_64_pc_windows_msvc = $env:RC
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $lldLink
    $env:RUSTFLAGS = "-Lnative=$xwinRoot\crt\lib\x86_64 -Lnative=$xwinRoot\sdk\lib\um\x86_64 -Lnative=$xwinRoot\sdk\lib\ucrt\x86_64"
    Write-Host "using local MSVC CRT/SDK shim: $xwinRoot"
}

$jdkBase = Join-Path $env:USERPROFILE '.tools\jdk17'
if (-not $env:JAVA_HOME -and (Test-Path $jdkBase)) {
    $java = Get-ChildItem $jdkBase -Recurse -Filter 'java.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like '*\bin\java.exe' } | Select-Object -First 1
    if ($java) { $env:JAVA_HOME = Split-Path -Parent (Split-Path -Parent $java.FullName) }
}

$sdkRoot = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
if (-not $env:ANDROID_HOME -and (Test-Path $sdkRoot)) { $env:ANDROID_HOME = $sdkRoot }
if (-not $env:NDK_HOME -and (Test-Path (Join-Path $sdkRoot 'ndk'))) {
    $ndk = Get-ChildItem (Join-Path $sdkRoot 'ndk') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($ndk) { $env:NDK_HOME = $ndk.FullName }
}

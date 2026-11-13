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

# Android cross compilation: point cc-rs (ring for rustls) and cargo at NDK clang.
if ($env:NDK_HOME) {
    $llvmBin = Join-Path $env:NDK_HOME 'toolchains\llvm\prebuilt\windows-x86_64\bin'
    if (Test-Path $llvmBin) {
        $targets = @{
            'aarch64_linux_android'           = 'aarch64-linux-android24-clang.cmd'
            'armv7_linux_androideabi'         = 'armv7a-linux-androideabi24-clang.cmd'
            'i686_linux_android'              = 'i686-linux-android24-clang.cmd'
            'x86_64_linux_android'            = 'x86_64-linux-android24-clang.cmd'
        }
        foreach ($key in $targets.Keys) {
            $clang = Join-Path $llvmBin $targets[$key]
            if (Test-Path $clang) {
                Set-Item -Path "env:CC_$key" -Value $clang
                Set-Item -Path "env:AR_$key" -Value (Join-Path $llvmBin 'llvm-ar.exe')
            }
        }
        $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $llvmBin 'aarch64-linux-android24-clang.cmd'
        $env:CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER = Join-Path $llvmBin 'armv7a-linux-androideabi24-clang.cmd'
        $env:CARGO_TARGET_I686_LINUX_ANDROID_LINKER = Join-Path $llvmBin 'i686-linux-android24-clang.cmd'
        $env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER = Join-Path $llvmBin 'x86_64-linux-android24-clang.cmd'
    }
}

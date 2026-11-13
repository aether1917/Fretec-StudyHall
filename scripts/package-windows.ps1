# Build the Windows NSIS installer into out/
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

& (Join-Path $PSScriptRoot 'seed-nsis.ps1')

Write-Host '== build frontend + Tauri NSIS installer =='
& pnpm tauri build --bundles nsis
if ($LASTEXITCODE -ne 0) { throw 'tauri build failed' }

$outDir = Join-Path $root 'out'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$bundleDir = Join-Path $root 'src-tauri\target\release\bundle\nsis'
$installers = Get-ChildItem $bundleDir -Filter '*.exe'
if (-not $installers) { throw "installer not found: $bundleDir" }
foreach ($installer in $installers) {
    Copy-Item $installer.FullName (Join-Path $outDir $installer.Name) -Force
    Write-Host "output out/$($installer.Name)"
}

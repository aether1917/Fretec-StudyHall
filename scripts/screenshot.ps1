# Capture a screenshot of the app window (no full desktop capture).
param(
    [string]$ProcessName = 'fretec-studyhall',
    [string]$OutputPath = 'out\screenshot.png',
    [int]$WaitSeconds = 6
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
public static class Win32 {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
"@

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $process) { throw "window not found for process $ProcessName" }

$handle = $process.MainWindowHandle
[Win32]::ShowWindow($handle, 5) | Out-Null
[Win32]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Seconds $WaitSeconds

$rect = New-Object RECT
if (-not [Win32]::GetWindowRect($handle, [ref]$rect)) { throw 'GetWindowRect failed' }
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) { throw 'invalid window rect' }

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)

$outFull = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputPath))
$dir = Split-Path -Parent $outFull
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
$bitmap.Save($outFull, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
Write-Host "screenshot saved: $outFull ($width x $height)"

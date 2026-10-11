# screenshot.ps1 -- capture a screen region to PNG (probe evidence, window-scoped)
# Usage: powershell -File tools/screenshot.ps1 -X 26 -Y 26 -W 320 -H 360 -Out shot.png
# DPI aware so the region matches physical pixels reported by win-probe.ps1.

param(
    [int]$X = 0,
    [int]$Y = 0,
    [int]$W = 320,
    [int]$H = 360,
    [string]$Out = "shot.png"
)

Add-Type @"
using System.Runtime.InteropServices;
public class DpiHelper { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware(); }
"@
[void][DpiHelper]::SetProcessDPIAware()

Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap($W, $H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($X, $Y, 0, 0, (New-Object System.Drawing.Size($W, $H)))
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "saved $Out ($W x $H at $X,$Y)"

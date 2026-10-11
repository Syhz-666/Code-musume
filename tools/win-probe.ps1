# win-probe.ps1 -- window inspector (shared tool for E2/E3)
# Usage: powershell -File tools/win-probe.ps1 -ProcessName pet
# Prints: every top-level window of the target process (HWND / visible / class / title /
#         rect / extended style with key flags decoded), plus the current foreground window.
# Note: keep this file ASCII-only -- Windows PowerShell 5.x reads BOM-less files as ANSI.

param(
    [string]$ProcessName = "pet"
)

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;

public class WinProbe
{
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassName(IntPtr hWnd, StringBuilder sb, int max);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder sb, int max);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    public static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int index);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern bool SetProcessDPIAware();

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

# DPI aware: GetWindowRect must return true physical pixels even when display scaling != 100%
[void][WinProbe]::SetProcessDPIAware()

function Describe-ExStyle([long]$ex) {
    $flags = @()
    if ($ex -band 0x00000008) { $flags += "TOPMOST" }
    if ($ex -band 0x00000020) { $flags += "TRANSPARENT" }
    if ($ex -band 0x00000080) { $flags += "TOOLWINDOW" }
    if ($ex -band 0x00080000) { $flags += "LAYERED" }
    if ($ex -band 0x08000000) { $flags += "NOACTIVATE" }
    if ($ex -band 0x00200000) { $flags += "NOREDIRECTIONBITMAP" }
    if ($flags.Count -eq 0) { $flags += "(none)" }
    return ($flags -join "|")
}

$procs = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
if (-not $procs) {
    Write-Output "[process '$ProcessName' not found]"
    exit 1
}
$pids = @($procs | ForEach-Object { $_.Id })
Write-Output "[$ProcessName PIDs: $($pids -join ', ')]"

$results = New-Object System.Collections.ArrayList
[WinProbe]::EnumWindows({
        param($h, $l)
        $pid2 = 0
        [void][WinProbe]::GetWindowThreadProcessId($h, [ref]$pid2)
        if ($pids -contains [int]$pid2) {
            $cls = New-Object System.Text.StringBuilder 256
            [void][WinProbe]::GetClassName($h, $cls, 256)
            $title = New-Object System.Text.StringBuilder 256
            [void][WinProbe]::GetWindowText($h, $title, 256)
            $rect = New-Object WinProbe+RECT
            [void][WinProbe]::GetWindowRect($h, [ref]$rect)
            $ex = [WinProbe]::GetWindowLongPtr($h, -20).ToInt64()

            [void]$results.Add([pscustomobject]@{
                    HWND    = ("0x{0:X}" -f $h.ToInt64())
                    Visible = [WinProbe]::IsWindowVisible($h)
                    Class   = $cls.ToString()
                    Title   = $title.ToString()
                    Rect    = "$($rect.Left),$($rect.Top) -> $($rect.Right),$($rect.Bottom) ($($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top))"
                    ExStyle = ("0x{0:X8}  {1}" -f $ex, (Describe-ExStyle $ex))
                })
        }
        return $true
    }, [IntPtr]::Zero) | Out-Null

if ($results.Count -eq 0) {
    Write-Output "  (no top-level windows)"
}
else {
    $results | Format-Table -AutoSize | Out-String -Width 300 | Write-Output
}

$fg = [WinProbe]::GetForegroundWindow()
$fgTitle = New-Object System.Text.StringBuilder 256
[void][WinProbe]::GetWindowText($fg, $fgTitle, 256)
$fgPid = 0
[void][WinProbe]::GetWindowThreadProcessId($fg, [ref]$fgPid)
Write-Output "foreground: pid=$fgPid title='$($fgTitle.ToString())'"

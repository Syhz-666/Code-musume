param([Parameter(Mandatory=$true)][int]$TargetProcessId)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class PetWindowProbe {
 public delegate bool Callback(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(Callback cb,IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll",EntryPoint="GetWindowLongPtrW")] public static extern IntPtr GetWindowLongPtr(IntPtr h,int i);
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
}
"@
[void][PetWindowProbe]::SetProcessDPIAware()
$windows=[Collections.Generic.List[object]]::new()
[void][PetWindowProbe]::EnumWindows({param($h,$l)
    $owner=0;[void][PetWindowProbe]::GetWindowThreadProcessId($h,[ref]$owner)
    if($owner -eq $TargetProcessId -and [PetWindowProbe]::IsWindowVisible($h)){
        $r=New-Object PetWindowProbe+Rect;[void][PetWindowProbe]::GetWindowRect($h,[ref]$r)
        $style=[PetWindowProbe]::GetWindowLongPtr($h,-20).ToInt64()
        $windows.Add([pscustomobject]@{left=$r.Left;top=$r.Top;width=$r.Right-$r.Left;height=$r.Bottom-$r.Top;noActivate=!!($style -band 0x08000000);topmost=!!($style -band 8);foreground=([PetWindowProbe]::GetForegroundWindow() -eq $h)})
    };return $true
},[IntPtr]::Zero)
$windows|ConvertTo-Json

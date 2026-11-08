# Sends real wheel notches to the GPUI prototype and captures the window
# after each one. Every notch is preceded by a check that the prototype's
# window is in the foreground; the run stops otherwise.
param(
    [Parameter(Mandatory)] [string] $Scene,
    [Parameter(Mandatory)] [string] $Script,
    [Parameter(Mandatory)] [string[]] $Steps,
    [Parameter(Mandatory)] [string] $OutDir
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class W {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, UIntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
'@
[W]::SetProcessDPIAware() | Out-Null

$exe = 'D:\code\rust\tTerm-P0-15-wheel\target\debug\tterm-gpui.exe'
$proc = Start-Process $exe -ArgumentList @('--exec', 'wsl.exe', '-d', 'Ubuntu', '--', 'bash', $Script) -PassThru
try {
    $deadline = (Get-Date).AddSeconds(30)
    do { Start-Sleep -Milliseconds 300; $proc.Refresh() } until ($proc.MainWindowHandle -ne 0 -or (Get-Date) -gt $deadline)
    $hwnd = $proc.MainWindowHandle
    if ($hwnd -eq 0) { throw "no window for $Scene" }
    Start-Sleep -Seconds 5

    function Focus-Prototype {
        [W]::ShowWindow($hwnd, 9) | Out-Null
        # A no-op Alt press lets this process move the foreground window.
        if ([W]::GetForegroundWindow() -ne $hwnd) {
            # Joining the foreground thread's input queue lifts the foreground
            # lock for this call; a no-op Alt press covers the remaining case.
            $fg = [W]::GetForegroundWindow()
            $pid0 = 0
            $fgThread = [W]::GetWindowThreadProcessId($fg, [ref]$pid0)
            $me = [W]::GetCurrentThreadId()
            [W]::AttachThreadInput($me, $fgThread, $true) | Out-Null
            [W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
            [W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
            [W]::BringWindowToTop($hwnd) | Out-Null
            [W]::SetForegroundWindow($hwnd) | Out-Null
            [W]::AttachThreadInput($me, $fgThread, $false) | Out-Null
            Start-Sleep -Milliseconds 500
        }
        $now = [W]::GetForegroundWindow()
        if ($now -ne $hwnd) {
            $owner = 0
            [W]::GetWindowThreadProcessId($now, [ref]$owner) | Out-Null
            $name = (Get-Process -Id $owner -ErrorAction SilentlyContinue).ProcessName
            throw "prototype $hwnd is not in the foreground (foreground $now belongs to $name/$owner); stopping before input"
        }
        $r = New-Object W+RECT
        [W]::GetWindowRect($hwnd, [ref]$r) | Out-Null
        [W]::SetCursorPos([int](($r.L + $r.R) / 2), [int](($r.T + $r.B) / 2)) | Out-Null
        return $r
    }

    function Capture([string] $name) {
        $r = Focus-Prototype
        $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
        $path = Join-Path $OutDir "$name.png"
        $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose()
        "captured $path"
    }

    Capture "$Scene-start"
    $i = 0
    foreach ($step in $Steps) {
        $i++
        # Positive WHEEL_DELTA (120) turns the wheel away from the user.
        $delta = if ($step -eq 'away') { 120 } else { -120 }
        Focus-Prototype | Out-Null
        [W]::mouse_event(0x0800, 0, 0, $delta, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 900
        Capture "$Scene-$i-$step"
    }
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}

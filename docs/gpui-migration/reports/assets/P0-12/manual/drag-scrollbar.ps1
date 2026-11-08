# Real-mouse scrollbar check for P0-12: drags the prototype's scrollbar thumb
# to the top, the middle and back to the bottom, capturing each step. Every
# press is preceded by a check that the prototype window is in the foreground.
param([Parameter(Mandatory)] [string] $OutDir)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class S {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, UIntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
'@
[S]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force $OutDir | Out-Null

$exe = 'D:\code\rust\tTerm-P0-12\target\debug\tterm-gpui.exe'
$script = '/mnt/c/Users/stone/AppData/Local/Temp/claude/1f05be85be30f9fa/9cce105a-1571-4e4f-a758-742a2ab00d4b/scratchpad/scrollbar/history.sh'
$proc = Start-Process $exe -ArgumentList @('--exec', 'wsl.exe', '-d', 'Ubuntu', '--', 'bash', $script) -PassThru
try {
    for ($i = 0; $i -lt 120 -and $proc.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 250; $proc.Refresh() }
    $h = $proc.MainWindowHandle
    if ($h -eq 0) { throw 'no prototype window' }
    Start-Sleep -Seconds 6

    function Focus {
        if ([S]::GetForegroundWindow() -eq $h) { return }
        $fg = [S]::GetForegroundWindow(); $p = 0
        $t = [S]::GetWindowThreadProcessId($fg, [ref]$p); $me = [S]::GetCurrentThreadId()
        [S]::AttachThreadInput($me, $t, $true) | Out-Null
        [S]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [S]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
        [S]::ShowWindow($h, 9) | Out-Null; [S]::BringWindowToTop($h) | Out-Null; [S]::SetForegroundWindow($h) | Out-Null
        [S]::AttachThreadInput($me, $t, $false) | Out-Null
        Start-Sleep -Milliseconds 500
        if ([S]::GetForegroundWindow() -ne $h) { throw 'prototype is not in the foreground; stopping before input' }
    }
    function Client {
        $c = New-Object S+RECT; [S]::GetClientRect($h, [ref]$c) | Out-Null
        $o = New-Object S+POINT; [S]::ClientToScreen($h, [ref]$o) | Out-Null
        @{ L = $o.X; T = $o.Y; W = $c.R; H = $c.B }
    }
    function Shot([string] $name) {
        $c = Client
        $bmp = New-Object System.Drawing.Bitmap $c.W, $c.H
        $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($c.L, $c.T, 0, 0, $bmp.Size)
        $bmp.Save((Join-Path $OutDir "$name.png"))
        $g.Dispose()
        return $bmp
    }
    # The thumb is the run of rows in the scrollbar column whose colour
    # differs from the track's most common colour.
    function Thumb([System.Drawing.Bitmap] $bmp) {
        $x = $bmp.Width - 5
        $counts = @{}
        for ($y = 0; $y -lt $bmp.Height; $y++) { $k = $bmp.GetPixel($x, $y).ToArgb(); $counts[$k] = 1 + [int]$counts[$k] }
        $track = ($counts.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 1).Key
        $rows = @(for ($y = 0; $y -lt $bmp.Height; $y++) { if ($bmp.GetPixel($x, $y).ToArgb() -ne $track) { $y } })
        if ($rows.Count -eq 0) { return $null }
        @{ Top = ($rows | Measure-Object -Minimum).Minimum; Bottom = ($rows | Measure-Object -Maximum).Maximum; X = $x }
    }
    function Drag([int] $fromY, [int] $toY) {
        $c = Client
        $x = $c.L + $c.W - 5
        Focus
        [S]::SetCursorPos($x, $c.T + $fromY) | Out-Null; Start-Sleep -Milliseconds 150
        if ([S]::GetForegroundWindow() -ne $h) { throw 'prototype lost the foreground; no press sent' }
        [S]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
        for ($i = 1; $i -le 20; $i++) {
            [S]::SetCursorPos($x, $c.T + [int]($fromY + ($toY - $fromY) * $i / 20)) | Out-Null
            Start-Sleep -Milliseconds 25
        }
        Start-Sleep -Milliseconds 200
        [S]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 600
    }

    Focus
    $bmp = Shot '1-bottom'
    $thumb = Thumb $bmp; $height = $bmp.Height; $bmp.Dispose()
    if (-not $thumb) { throw 'no scrollbar thumb found' }
    "thumb at bottom: {0}-{1} of {2}" -f $thumb.Top, $thumb.Bottom, $height
    $mid = [int](($thumb.Top + $thumb.Bottom) / 2)

    Drag $mid ([int]($height / 2))
    $bmp = Shot '2-middle'; $bmp.Dispose()
    $c = [int]($height / 2)
    Drag $c 2
    $bmp = Shot '3-top'; $bmp.Dispose()
    Drag 9 ($height - 2)
    $bmp = Shot '4-bottom-again'; $t = Thumb $bmp; $bmp.Dispose(); "after drag to bottom: {0}-{1}" -f $t.Top, $t.Bottom
} finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}



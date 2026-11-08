# Real-mouse drag check for the P0-19 drag-out probe. Drags the probe's file
# into an Explorer window showing a new empty TEMP directory, or cancels the
# drag with Escape. Stops before any input when the expected window is not
# where it should be.
param(
    [Parameter(Mandatory)] [ValidateSet('drop', 'escape')] [string] $Mode,
    [Parameter(Mandatory)] [string] $OutDir
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class D {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hgt, bool r);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hgt, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, UIntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
'@
[D]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force $OutDir | Out-Null

function Shot([IntPtr] $h, [string] $name) {
    $r = New-Object D+RECT; [D]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $OutDir "$name.png")); $g.Dispose(); $bmp.Dispose()
}
function Focus([IntPtr] $h) {
    if ([D]::GetForegroundWindow() -eq $h) { return }
    $fg = [D]::GetForegroundWindow(); $p = 0
    $t = [D]::GetWindowThreadProcessId($fg, [ref]$p); $me = [D]::GetCurrentThreadId()
    [D]::AttachThreadInput($me, $t, $true) | Out-Null
    [D]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [D]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
    [D]::BringWindowToTop($h) | Out-Null; [D]::SetForegroundWindow($h) | Out-Null
    [D]::AttachThreadInput($me, $t, $false) | Out-Null
    Start-Sleep -Milliseconds 500
}
function TopAt([int] $x, [int] $y) {
    $pt = New-Object D+POINT; $pt.X = $x; $pt.Y = $y
    [D]::GetAncestor([D]::WindowFromPoint($pt), 2)
}

$target = Join-Path $env:TEMP ('tTerm-P0-19-target-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $target | Out-Null
"target=$target"

# Explorer: open the new empty directory and find its window through the shell.
$shell = New-Object -ComObject Shell.Application
$before = @($shell.Windows() | ForEach-Object { $_.HWND })
Start-Process explorer.exe $target
$explorer = $null
for ($i = 0; $i -lt 40 -and -not $explorer; $i++) {
    Start-Sleep -Milliseconds 250
    $explorer = $shell.Windows() | Where-Object { $_.HWND -notin $before -and $_.Document.Folder.Self.Path -eq $target } | Select-Object -First 1
}
if (-not $explorer) { throw "no Explorer window for $target" }
$ex = [IntPtr]$explorer.HWND
[D]::ShowWindow($ex, 9) | Out-Null
[D]::MoveWindow($ex, 900, 80, 800, 600, $true) | Out-Null
# Keep this script's own Explorer window above the others for the drop; the
# foreground lock can refuse a plain raise, and the window is closed after.
[D]::SetWindowPos($ex, [IntPtr](-1), 900, 80, 800, 600, 0x0040) | Out-Null

# Probe: start with the target and keep its output.
$exe = 'D:\code\rust\tTerm-P0-19\target\debug\examples\p0_19_drag_out.exe'
$log = Join-Path $OutDir "$Mode-stdout.txt"
$probe = Start-Process $exe -ArgumentList @('--target', $target) -RedirectStandardOutput $log -RedirectStandardError (Join-Path $OutDir "$Mode-stderr.txt") -PassThru
try {
    for ($i = 0; $i -lt 60 -and $probe.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 250; $probe.Refresh() }
    $ph = $probe.MainWindowHandle
    if ($ph -eq 0) { throw 'no probe window' }
    Start-Sleep -Seconds 2
    Focus $ph
    if ([D]::GetForegroundWindow() -ne $ph) { throw 'probe is not in the foreground; no input sent' }
    Shot $ph "$Mode-1-before"

    # Find the blue drag area (#345779) in the probe window.
    $r = New-Object D+RECT; [D]::GetWindowRect($ph, [ref]$r) | Out-Null
    $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $xs = @(); $ys = @()
    for ($y = 0; $y -lt $bmp.Height; $y += 3) { for ($x = 0; $x -lt $bmp.Width; $x += 3) {
        $c = $bmp.GetPixel($x, $y); if ($c.R -eq 0x34 -and $c.G -eq 0x57 -and $c.B -eq 0x79) { $xs += $x; $ys += $y } } }
    $g.Dispose(); $bmp.Dispose()
    if ($xs.Count -eq 0) { throw 'blue drag area not found' }
    $sx = $r.L + [int](($xs | Measure-Object -Minimum).Minimum + 40)
    $sy = $r.T + [int]((($ys | Measure-Object -Minimum).Minimum + ($ys | Measure-Object -Maximum).Maximum) / 2)
    $dx = 1300; $dy = 380
    $atDrop = TopAt $dx $dy
    if ($atDrop -ne $ex) {
        $owner = 0; [D]::GetWindowThreadProcessId($atDrop, [ref]$owner) | Out-Null
        $er = New-Object D+RECT; [D]::GetWindowRect($ex, [ref]$er) | Out-Null
        throw ("drop point is over {0} of {1}, not Explorer {2} at {3},{4}-{5},{6}; probe at {7},{8}-{9},{10}; no input sent" -f `
            $atDrop, (Get-Process -Id $owner).ProcessName, $ex, $er.L, $er.T, $er.R, $er.B, $r.L, $r.T, $r.R, $r.B)
    }

    # Press, move in small steps past the threshold, then across to Explorer.
    [D]::SetCursorPos($sx, $sy) | Out-Null; Start-Sleep -Milliseconds 200
    if ([D]::GetForegroundWindow() -ne $ph) { throw 'probe lost the foreground; no input sent' }
    [D]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le 40; $i++) {
        [D]::SetCursorPos([int]($sx + ($dx - $sx) * $i / 40), [int]($sy + ($dy - $sy) * $i / 40)) | Out-Null
        Start-Sleep -Milliseconds 30
    }
    Start-Sleep -Milliseconds 600
    if ($Mode -eq 'escape') {
        [D]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero); [D]::keybd_event(0x1B, 0, 2, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 300
        [D]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    } else {
        if ((TopAt $dx $dy) -ne $ex) { [D]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero); throw 'cursor left the target window; released early' }
        [D]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    }

    # Wait for the probe to finish publishing (or for the cancel to settle).
    $deadline = (Get-Date).AddSeconds(180)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 2
        $text = Get-Content $log -Raw -ErrorAction SilentlyContinue
        if ($text -match 'Published and verified|Failed|rejected|cancel|accepted=false') { break }
    }
    Start-Sleep -Seconds 3
    Shot $ph "$Mode-2-probe-after"
    Shot $ex "$Mode-3-explorer-after"
    "--- target contents"
    Get-ChildItem $target -Force | ForEach-Object { "{0}  {1:N0} bytes" -f $_.Name, $_.Length }
    "--- probe stdout"
    Get-Content $log -ErrorAction SilentlyContinue
} finally {
    if (-not $probe.HasExited) { Stop-Process -Id $probe.Id -Force }
    try { $explorer.Quit() } catch {}
    Remove-Item -Recurse -Force $target -ErrorAction SilentlyContinue
}

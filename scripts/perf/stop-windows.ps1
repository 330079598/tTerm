$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$launch = Get-Content -Raw (Join-Path $repo 'target/perf/launch.json') | ConvertFrom-Json
$expectedData = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'com.stone.tTerm.perf'
$expectedEmpty = Join-Path $repo 'target/perf/empty-appdata'
if ([IO.Path]::GetFullPath($launch.data) -ne [IO.Path]::GetFullPath($expectedData) -or
    [IO.Path]::GetFullPath($launch.temporary_appdata) -ne [IO.Path]::GetFullPath($expectedEmpty)) {
    throw 'Unexpected cleanup target; refusing to stop or delete.'
}
$processes = @(Get-CimInstance Win32_Process)
$root = $processes | Where-Object { $_.ProcessId -eq $launch.pid }
if ($root -and $root.ExecutablePath -ne (Join-Path $repo 'target/release/tTerm.exe')) {
    throw 'PID was reused; refusing to stop.'
}
$ids = [System.Collections.Generic.HashSet[int]]::new()
[void]$ids.Add([int]$launch.pid)
do {
    $added = $false
    foreach ($process in $processes) {
        if ($ids.Contains([int]$process.ParentProcessId) -and $process.CreationDate -ge $root.CreationDate) {
            if ($ids.Add([int]$process.ProcessId)) { $added = $true }
        }
    }
} while ($added)
foreach ($id in $ids) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2
$targets = @($expectedData, $expectedEmpty,
    (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'com.stone.tTerm.perf'))
foreach ($target in $targets) {
    $absolute = [IO.Path]::GetFullPath($target)
    if ($absolute -notin @([IO.Path]::GetFullPath($expectedData), [IO.Path]::GetFullPath($expectedEmpty),
        [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'com.stone.tTerm.perf')))) {
        throw "Unapproved cleanup path: $absolute"
    }
    if (Test-Path -LiteralPath $absolute) {
        $item = Get-Item -LiteralPath $absolute
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing reparse point cleanup.' }
        Remove-Item -LiteralPath $absolute -Recurse -Force
    }
    Write-Output "Removed/absent: $absolute"
}

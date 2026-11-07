$ErrorActionPreference = 'Stop'
$evidence = 'D:\code\rust\tTerm-p0-26-scratch\evidence'
$stopFile = 'D:\code\rust\tTerm-p0-26-scratch\stop-watcher'
$seen = @{}
while (!(Test-Path -LiteralPath $stopFile)) {
    $processes = Get-CimInstance Win32_Process -Filter "Name LIKE '%GPUI%' OR Name = 'uninstall.exe'"
    foreach ($process in $processes) {
        $path = $process.ExecutablePath
        if (!$path -or ($path -notmatch '(?i)tTerm[ -]GPUI[ -]Dev')) { continue }
        $key = "$($process.ProcessId):$($process.CreationDate)"
        if ($seen.ContainsKey($key)) { continue }
        $seen[$key] = $true
        [ordered]@{
            time = [DateTime]::UtcNow.ToString('o')
            pid = $process.ProcessId; parent = $process.ParentProcessId
            executable = $path; arguments = $process.CommandLine
        } | ConvertTo-Json -Compress | Add-Content -LiteralPath (Join-Path $evidence 'update-processes.jsonl')
    }
    Start-Sleep -Milliseconds 250
}

param(
    [Parameter(Mandatory)][int]$RootPid,
    [Parameter(Mandatory)][string]$Workload,
    [Parameter(Mandatory)][string]$Output,
    [int]$Seconds = 40,
    [string]$Build = 'release-tauri-perf'
)
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 2) { throw 'Seconds must be at least 2.' }
$logical = (Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
$root = Get-CimInstance Win32_Process -Filter "ProcessId=$RootPid"
if (!$root) { throw 'Root process is not running.' }
$known = @{ $RootPid = $root.CreationDate }
$rows = [System.Collections.Generic.List[object]]::new()
$clock = [Diagnostics.Stopwatch]::StartNew()
$paths = @('\Process(*)\ID Process', '\Process(*)\% Processor Time',
    '\Process(*)\Working Set', '\Process(*)\Working Set - Private', '\Process(*)\Private Bytes')
$gpuAvailable = $true
$gpuError = ''
while ($clock.Elapsed.TotalSeconds -lt $Seconds) {
    $sampleStart = $clock.Elapsed.TotalSeconds
    $processes = @(Get-CimInstance Win32_Process)
    do {
        $added = $false
        foreach ($process in $processes) {
            $id = [int]$process.ProcessId
            $parent = [int]$process.ParentProcessId
            if (!$known.ContainsKey($id) -and $known.ContainsKey($parent) -and
                $process.CreationDate -ge $known[$parent]) {
                $known[$id] = $process.CreationDate
                $added = $true
            }
        }
    } while ($added)
    $members = @($processes | Where-Object {
        $known.ContainsKey([int]$_.ProcessId) -and $known[[int]$_.ProcessId] -eq $_.CreationDate
    })
    $ids = @($members | ForEach-Object { [int]$_.ProcessId })
    $counter = Get-Counter -Counter $paths -ErrorAction SilentlyContinue
    if (!$counter) { throw 'Process performance counters unavailable; no fallback numbers are fabricated.' }
    $instances = @($counter.CounterSamples | Where-Object {
        $_.Path.EndsWith('\id process') -and [int]$_.CookedValue -in $ids
    } | ForEach-Object { $_.Path.Substring(0, $_.Path.LastIndexOf('\')) })
    if (!$instances.Count) { throw 'No performance counter instances matched the application tree.' }
    $values = @{}
    foreach ($name in @('% processor time', 'working set', 'working set - private', 'private bytes')) {
        $matching = @($counter.CounterSamples | Where-Object {
            $_.Path.Substring(0, $_.Path.LastIndexOf('\')) -in $instances -and
                $_.Path.EndsWith("\$name") -and $_.Status -eq 0
        })
        $values[$name] = ($matching | Measure-Object CookedValue -Sum).Sum
    }
    $gpu = $null
    if ($gpuAvailable) {
        try {
            $gpuCounter = Get-Counter -Counter '\GPU Process Memory(*)\Dedicated Usage', '\GPU Process Memory(*)\Shared Usage' -ErrorAction SilentlyContinue
            if (!$gpuCounter) { throw 'GPU performance counter query returned no data.' }
            $gpu = @($gpuCounter.CounterSamples | Where-Object {
                $_.InstanceName -match '^pid_(\d+)_' -and [int]$Matches[1] -in $ids -and $_.Status -eq 0
            })
            if (!$gpu.Count) { throw 'No valid GPU process memory samples for this application tree.' }
        } catch { $gpuAvailable = $false; $gpuError = $_.Exception.Message }
    }
    $row = [ordered]@{
        utc = [DateTime]::UtcNow.ToString('o'); elapsed_s = $sampleStart
        cpu_percent_machine = $values['% processor time'] / $logical
        working_set_bytes = $values['working set']
        private_working_set_bytes = $values['working set - private']
        private_bytes = $values['private bytes']
        gpu_dedicated_bytes = $null; gpu_shared_bytes = $null
        members = @($members | Select-Object ProcessId, ParentProcessId, Name, CommandLine)
        counter_instances = $instances
    }
    if ($gpuAvailable) {
        $row.gpu_dedicated_bytes = ($gpu | Where-Object { $_.Path.EndsWith('\dedicated usage') } | Measure-Object CookedValue -Sum).Sum
        $row.gpu_shared_bytes = ($gpu | Where-Object { $_.Path.EndsWith('\shared usage') } | Measure-Object CookedValue -Sum).Sum
    }
    $rows.Add([pscustomobject]$row)
    $remaining = 1000 - ($clock.Elapsed.TotalSeconds - $sampleStart) * 1000
    if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int]$remaining) }
}
$metadata = @{ platform = 'windows'; build = $Build; workload = $Workload
    root_pid = $RootPid; logical_processors = $logical; requested_seconds = $Seconds
    gpu_error = $gpuError; samples = $rows }
$metadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Output -Encoding utf8
Write-Output "Captured $($rows.Count) samples: $Workload -> $Output"

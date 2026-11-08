param([int]$Port = 9341)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$originalAppData = [Environment]::GetFolderPath('ApplicationData')
$data = Join-Path $originalAppData 'com.stone.tTerm.perf'
$empty = Join-Path $repo 'target/perf/empty-appdata'
if (Test-Path -LiteralPath $data) { throw "Refusing to reuse existing isolated data: $data" }
if (Test-Path -LiteralPath $empty) { throw "Refusing to reuse temporary APPDATA: $empty" }
New-Item -ItemType Directory -Path $empty | Out-Null
$priorAppData = $env:APPDATA
$priorArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
try {
    $env:APPDATA = $empty
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $process = Start-Process -FilePath (Join-Path $repo 'target/release/tTerm.exe') -PassThru -WindowStyle Hidden
    @{ pid = $process.Id; data = $data; temporary_appdata = $empty; port = $Port
       original_appdata = $originalAppData; localappdata = $env:LOCALAPPDATA } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repo 'target/perf/launch.json') -Encoding utf8
    Write-Output "Started isolated release PID $($process.Id); verify empty profiles/session over CDP before input."
} finally {
    $env:APPDATA = $priorAppData
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $priorArguments
}

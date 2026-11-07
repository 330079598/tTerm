param([switch]$Build)
$ErrorActionPreference = 'Stop'
$repository = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$scratch = 'D:\code\rust\tTerm-p0-26-scratch'
$buildRoot = Join-Path $scratch 'tauri-build'
$configurationPath = Join-Path $scratch 'tauri.gpui-dev.generated.conf.json'

foreach ($name in @('TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PATH', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD')) {
    [Environment]::SetEnvironmentVariable($name, $null, 'Process')
}
$configuration = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'tauri.gpui-dev.conf.json') -Raw | ConvertFrom-Json
$configuration.plugins.updater.pubkey = (Get-Content -LiteralPath 'D:\code\rust\tTerm-p0-26-keys\test.key.pub' -Raw).Trim()
$configuration | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $configurationPath -Encoding utf8NoBOM

Push-Location $buildRoot
try {
    & git apply --reverse --check (Join-Path $PSScriptRoot 'experiment-updater.patch')
    if ($LASTEXITCODE -ne 0) { throw 'The reviewed experiment patch is not applied.' }
    $base = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json -AsHashtable
    $windows = Get-Content src-tauri/tauri.windows.conf.json -Raw | ConvertFrom-Json -AsHashtable
    $overlay = Get-Content $configurationPath -Raw | ConvertFrom-Json -AsHashtable
    function Merge-Configuration($Target, $Source) {
        foreach ($key in $Source.Keys) {
            if ($Source[$key] -is [System.Collections.IDictionary] -and $Target[$key] -is [System.Collections.IDictionary]) {
                Merge-Configuration $Target[$key] $Source[$key]
            } else { $Target[$key] = $Source[$key] }
        }
    }
    Merge-Configuration $base $windows
    Merge-Configuration $base $overlay
    if ($base.identifier -ne 'com.stone.tTerm.gpui-dev' -or $base.productName -ne 'tTerm GPUI Dev' -or
        $base.version -ne '0.0.1' -or $base.bundle.createUpdaterArtifacts -ne $false -or
        $null -ne $base.bundle.windows.nsis.installerHooks) { throw 'Unsafe experiment configuration.' }
    $base | ConvertTo-Json -Depth 30 | Set-Content (Join-Path $scratch 'evidence/tauri-merged-preflight.json') -Encoding utf8NoBOM
    if ($Build) {
        pnpm install
        if ($LASTEXITCODE -ne 0) { throw 'Temporary worktree pnpm install failed.' }
        $env:CARGO_TARGET_DIR = Join-Path $scratch 'target-tauri'
        pnpm tauri build --config $configurationPath --bundles nsis --verbose
        if ($LASTEXITCODE -ne 0) { throw 'Tauri experiment build failed.' }
    }
} finally { Pop-Location }

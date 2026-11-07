$ErrorActionPreference = 'Stop'
$repository = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$scratch = 'D:\code\rust\tTerm-p0-26-scratch'
$stage = Join-Path $scratch 'staging'
foreach ($name in @('CARGO_PACKAGER_SIGN_PRIVATE_KEY', 'CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD')) {
    [Environment]::SetEnvironmentVariable($name, $null, 'Process')
}
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Push-Location $repository
try {
    $env:CARGO_TARGET_DIR = 'D:\code\rust\tTerm-build-P0-13'
    cargo build --workspace --release --bin tterm-gpui
    if ($LASTEXITCODE -ne 0) { throw 'GPUI release build failed.' }
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/tterm-gpui.exe') -Destination (Join-Path $stage 'tTerm GPUI Dev.exe')
    & ./scripts/prepare-conpty.ps1 -Architecture x86_64 -TestDirectory $stage
    Copy-Item -LiteralPath src-tauri/resources/conpty-LICENSE.txt -Destination (Join-Path $stage 'conpty-LICENSE.txt')
    $configuration = @{
        name = 'tterm-gpui-dev'
        productName = 'tTerm GPUI Dev'; version = '0.0.2'
        identifier = 'com.stone.tTerm.gpui-dev'; publisher = 'stone'
        formats = @('nsis'); outDir = (Join-Path $scratch 'packages'); binariesDir = $stage
        binaries = @(@{ path = 'tTerm GPUI Dev'; main = $true })
        targetTriple = 'x86_64-pc-windows-msvc'; description = 'tTerm GPUI Dev packaging experiment'
        resources = @('conpty.dll', 'OpenConsole.exe', 'conpty-LICENSE.txt') | ForEach-Object {
            @{ src = (Join-Path $stage $_); target = $_ }
        }
        nsis = @{
            installMode = 'currentUser'; languages = @('English')
            installerIcon = (Join-Path $repository 'src-tauri/icons/icon.ico')
        }
    }
    $configurationPath = Join-Path $scratch 'packager.gpui-dev.json'
    $configuration | ConvertTo-Json -Depth 20 | Set-Content $configurationPath -Encoding utf8NoBOM
    & (Join-Path $scratch 'tools/bin/cargo-packager.exe') --config $configurationPath -vv
    if ($LASTEXITCODE -ne 0) { throw 'GPUI packaging failed.' }
} finally { Pop-Location }

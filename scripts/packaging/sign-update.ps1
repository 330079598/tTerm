$ErrorActionPreference = 'Stop'
foreach ($name in @('TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PATH', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD')) {
    [Environment]::SetEnvironmentVariable($name, $null, 'Process')
}
$scratch = 'D:\code\rust\tTerm-p0-26-scratch'
$package = Join-Path $scratch 'packages/tTerm GPUI Dev_0.0.2_x64-setup.exe'
pnpm tauri signer sign --private-key-path D:/code/rust/tTerm-p0-26-keys/test.key --app-version 0.0.2 $package
if ($LASTEXITCODE -ne 0) { throw 'GPUI signing failed.' }
$serverRoot = Join-Path $scratch 'server'
New-Item -ItemType Directory -Path $serverRoot -Force | Out-Null
Copy-Item -LiteralPath $package -Destination (Join-Path $serverRoot 'tTerm-GPUI-Dev_0.0.2_x64-setup.exe')
$manifest = @{
    version = '0.0.2'
    notes = 'P0-26 local Windows experiment: replace Tauri GPUI Dev 0.0.1 with GPUI terminal 0.0.2.'
    pub_date = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = @{}
}
foreach ($key in @('windows-x86_64', 'windows-x86_64-nsis')) {
    $manifest.platforms[$key] = @{
        url = 'http://127.0.0.1:18226/tTerm-GPUI-Dev_0.0.2_x64-setup.exe'
        signature = (Get-Content -LiteralPath "$package.sig" -Raw).Trim()
    }
}
$manifest | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $serverRoot 'latest.json') -Encoding utf8NoBOM

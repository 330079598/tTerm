param([ValidateSet('before', 'tauri-installed', 'gpui-updated', 'after')][string]$Phase)
$ErrorActionPreference = 'Stop'
$snapshotRoot = 'D:\code\rust\tTerm-p0-26-scratch\evidence\system'
$snapshotDir = Join-Path $snapshotRoot $Phase
New-Item -ItemType Directory -Path $snapshotDir -Force | Out-Null

function Read-RegistryTree([string]$Key) {
    if (Test-Path -LiteralPath $Key) {
        $nativeKey = $Key.Replace('Registry::', '')
        & reg.exe query $nativeKey /s
        if ($LASTEXITCODE -ne 0) { throw "Registry query failed: $Key" }
    } else {
        "ABSENT: $Key"
    }
}

$uninstallRoots = @(
    'Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall',
    'Registry::HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\Uninstall',
    'Registry::HKEY_LOCAL_MACHINE\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'
)
$(foreach ($uninstallRoot in $uninstallRoots) {
    "ROOT: $uninstallRoot"
    $uninstallKeys = @(Get-ChildItem -LiteralPath $uninstallRoot | Where-Object {
        $_.PSChildName -match 'tTerm' -or $_.GetValue('DisplayName') -match 'tTerm'
    } | Sort-Object Name)
    if ($uninstallKeys.Count -eq 0) { 'NO MATCHING UNINSTALL KEYS' }
    foreach ($key in $uninstallKeys) { Read-RegistryTree "Registry::$($key.Name)" }
}) | Set-Content -LiteralPath (Join-Path $snapshotDir 'uninstall.txt') -Encoding utf8

# The installer records the install directory under Software\<manufacturer>\<product>.
# The manufacturer is expected to be 'stone'; add the real one if the generated scripts differ.
$(foreach ($vendorKey in @(
    'Registry::HKEY_CURRENT_USER\Software\stone',
    'Registry::HKEY_LOCAL_MACHINE\Software\stone'
)) {
    "ROOT: $vendorKey"
    if (!(Test-Path -LiteralPath $vendorKey)) { 'ABSENT'; continue }
    $productKeys = @(Get-ChildItem -LiteralPath $vendorKey |
        Where-Object { $_.PSChildName -match 'tTerm' } | Sort-Object Name)
    if ($productKeys.Count -eq 0) { 'NO MATCHING PRODUCT KEYS' }
    foreach ($key in $productKeys) { Read-RegistryTree "Registry::$($key.Name)" }
}) | Set-Content -LiteralPath (Join-Path $snapshotDir 'install-location.txt') -Encoding utf8

$(foreach ($branch in @('Directory\shell', 'Directory\Background\shell', 'Drive\shell')) {
    $menuKey = "Registry::HKEY_CURRENT_USER\Software\Classes\$branch\tTerm"
    Read-RegistryTree $menuKey
}) | Set-Content -LiteralPath (Join-Path $snapshotDir 'menus.txt') -Encoding utf8

$environmentKey = Get-Item -LiteralPath 'Registry::HKEY_CURRENT_USER\Environment'
$(if ($environmentKey.GetValueNames() -contains 'Path') {
    "Kind: $($environmentKey.GetValueKind('Path'))"
    $environmentKey.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
} else { 'ABSENT: HKCU\Environment\Path' }
) | Set-Content -LiteralPath (Join-Path $snapshotDir 'user-path.txt') -Encoding utf8

$shortcutShell = New-Object -ComObject WScript.Shell
# Desktops are listed without recursion so that unrelated user folders are not walked.
$shortcutRoots = @(
    @{ Path = [Environment]::GetFolderPath('Programs'); Recurse = $true },
    @{ Path = [Environment]::GetFolderPath('CommonPrograms'); Recurse = $true },
    @{ Path = [Environment]::GetFolderPath('Desktop'); Recurse = $false },
    @{ Path = [Environment]::GetFolderPath('CommonDesktopDirectory'); Recurse = $false }
)
$(foreach ($shortcutRoot in $shortcutRoots) {
    "ROOT: $($shortcutRoot.Path)"
    if (!(Test-Path -LiteralPath $shortcutRoot.Path)) { 'ABSENT'; continue }
    $links = @(Get-ChildItem -LiteralPath $shortcutRoot.Path -Recurse:$shortcutRoot.Recurse -File -Filter '*.lnk' |
        Where-Object { $_.Name -match 'tTerm' } | Sort-Object FullName)
    if ($links.Count -eq 0) { 'NO MATCHING SHORTCUTS' }
    foreach ($link in $links) {
        $shortcut = $shortcutShell.CreateShortcut($link.FullName)
        [ordered]@{
            Path = $link.FullName
            SHA256 = (Get-FileHash -LiteralPath $link.FullName -Algorithm SHA256).Hash
            Target = $shortcut.TargetPath
            Arguments = $shortcut.Arguments
            WorkingDirectory = $shortcut.WorkingDirectory
            Icon = $shortcut.IconLocation
        } | ConvertTo-Json -Compress
    }
}) | Set-Content -LiteralPath (Join-Path $snapshotDir 'shortcuts.txt') -Encoding utf8

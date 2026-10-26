param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('x86_64', 'aarch64', 'x86')]
    [string]$Architecture,
    [Parameter(Mandatory = $true)]
    [string]$TestDirectory
)

$ErrorActionPreference = 'Stop'

function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

# Update the version and archive hash together when upgrading the bundled runtime.
$package = @{
    version = '1.25.260930003'
    sha256 = '02b07b349af66d801159bdf9e440d4a1ce78bb951f37fc8609731665afdae7ee'
}
$arch = @{ x86_64 = 'x64'; aarch64 = 'arm64'; x86 = 'x86' }[$Architecture]
$destination = Join-Path $PSScriptRoot '../src-tauri/resources/conpty'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
New-Item -ItemType Directory -Path $TestDirectory -Force | Out-Null
$archive = Join-Path $destination "conpty-$($package.version).zip"

# A pinned archive is reusable offline; never trust an incomplete or changed download.
if (!(Test-Path -LiteralPath $archive)) {
    $download = "$archive.download"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $url = "https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/$($package.version)/microsoft.windows.console.conpty.$($package.version).nupkg"
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $download
    if ((Get-Sha256 $download) -ne $package.sha256) {
        throw 'ConPTY download checksum mismatch. Remove the .download file and retry.'
    }
    Move-Item -LiteralPath $download -Destination $archive -Force
}
if ((Get-Sha256 $archive) -ne $package.sha256) {
    throw "ConPTY cache checksum mismatch. Remove $archive and rebuild."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    $entries = @{
        'conpty.dll' = "runtimes/win-$arch/native/conpty.dll"
        'OpenConsole.exe' = "build/native/runtimes/$arch/OpenConsole.exe"
    }
    foreach ($name in $entries.Keys) {
        $entry = $zip.GetEntry($entries[$name])
        if ($null -eq $entry) { throw "ConPTY package is missing $($entries[$name])" }
        $staged = Join-Path $destination "$name.new"
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $staged, $true)
        foreach ($directory in @($destination, $TestDirectory)) {
            $target = Join-Path $directory $name
            # Do not rewrite a loaded DLL when another dev/test process is running.
            if (!(Test-Path -LiteralPath $target) -or
                (Get-Sha256 $target) -ne (Get-Sha256 $staged)) {
                Copy-Item -LiteralPath $staged -Destination $target -Force
            }
        }
        Remove-Item -LiteralPath $staged
    }
} finally {
    $zip.Dispose()
}

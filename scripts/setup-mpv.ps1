[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Upstream-referenced third-party build: https://mpv.io/installation/
# Pin both archive and extracted DLL; never silently substitute a newer build.
$mpvRelease = '20261111'
$mpvArchive = 'mpv-dev-x86_64-20261111-git-bcde6b93cc.7z'
$mpvUrl = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/$mpvRelease/$mpvArchive"
$mpvArchiveSha256 = '273c079b11ea5575822f972756f4c9a71149a5c10130353abeb8586738938452'
$mpvDllSha256 = '88504ee5560942a955405e99dfb5971368e4cf851321f36565bf2a667271db1b'
$mpvRoot = Split-Path -Parent $PSScriptRoot
$mpvDestination = Join-Path $mpvRoot 'src-tauri\resources\mpv'
$mpvDll = Join-Path $mpvDestination 'libmpv-2.dll'

New-Item -ItemType Directory -Path $mpvDestination -Force | Out-Null
if ((Test-Path -LiteralPath $mpvDll) -and ((Get-FileHash -LiteralPath $mpvDll -Algorithm SHA256).Hash.ToLowerInvariant() -eq $mpvDllSha256)) {
    Write-Host "libmpv $mpvRelease is already installed and verified."
    exit 0
}

$mpvWork = Join-Path ([System.IO.Path]::GetTempPath()) ('AudioPlayer-mpv-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $mpvWork | Out-Null
$mpvDownload = Join-Path $mpvWork $mpvArchive
Write-Host "Downloading pinned libmpv $mpvRelease (Windows x64 baseline)..."
Invoke-WebRequest -Uri $mpvUrl -OutFile $mpvDownload
if ((Get-FileHash -LiteralPath $mpvDownload -Algorithm SHA256).Hash.ToLowerInvariant() -ne $mpvArchiveSha256) {
    throw 'Downloaded libmpv archive failed SHA256 verification.'
}

$mpvTar = Get-Command tar.exe -ErrorAction SilentlyContinue
if (-not $mpvTar) { throw 'Windows tar.exe is required to extract the .7z archive.' }
& $mpvTar.Source -xf $mpvDownload -C $mpvWork libmpv-2.dll
if ($LASTEXITCODE -ne 0) { throw 'Cannot extract libmpv-2.dll.' }
$mpvExtracted = Join-Path $mpvWork 'libmpv-2.dll'
if ((Get-FileHash -LiteralPath $mpvExtracted -Algorithm SHA256).Hash.ToLowerInvariant() -ne $mpvDllSha256) {
    throw 'Extracted libmpv DLL failed SHA256 verification.'
}
Copy-Item -LiteralPath $mpvExtracted -Destination $mpvDll -Force
Remove-Item -LiteralPath $mpvExtracted
Remove-Item -LiteralPath $mpvDownload
Remove-Item -LiteralPath $mpvWork
Write-Host "Verified libmpv is ready: $mpvDll"

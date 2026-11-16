[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Windows binary vendor linked by https://ffmpeg.org/download.html.
# Both the immutable release archive and extracted runtime are pinned.
$audioToolsVersion = '9.0.2'
$audioToolsArchive = 'ffmpeg-9.0.2-essentials_build.zip'
$audioToolsUrl = "https://www.gyan.dev/ffmpeg/builds/packages/$audioToolsArchive"
$audioToolsArchiveHash = '60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba'
$audioToolsExecutableHash = '3256173f3f8bffd7df12227c68adf68025edb1832273a9530688a7bb1ed8edec'
$audioToolsRoot = Split-Path -Parent $PSScriptRoot
$audioToolsDestination = Join-Path $audioToolsRoot 'src-tauri\resources\ffmpeg'
$audioToolsExecutable = Join-Path $audioToolsDestination 'ffmpeg.exe'
New-Item -ItemType Directory -Path $audioToolsDestination -Force | Out-Null
if ((Test-Path -LiteralPath $audioToolsExecutable) -and ((Get-FileHash -LiteralPath $audioToolsExecutable -Algorithm SHA256).Hash.ToLowerInvariant() -eq $audioToolsExecutableHash)) {
    Write-Host "FFmpeg $audioToolsVersion is already installed and verified."
    exit 0
}
$audioToolsWork = Join-Path ([IO.Path]::GetTempPath()) ('SongPlayer-ffmpeg-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $audioToolsWork | Out-Null
$audioToolsDownload = Join-Path $audioToolsWork $audioToolsArchive
Write-Host "Downloading pinned FFmpeg $audioToolsVersion (Windows x64)..."
Invoke-WebRequest -Uri $audioToolsUrl -OutFile $audioToolsDownload
if ((Get-FileHash -LiteralPath $audioToolsDownload -Algorithm SHA256).Hash.ToLowerInvariant() -ne $audioToolsArchiveHash) { throw 'FFmpeg archive failed SHA256 verification.' }
$audioToolsPackage = 'ffmpeg-9.0.2-essentials_build'
& tar.exe -xf $audioToolsDownload -C $audioToolsWork "$audioToolsPackage/bin/ffmpeg.exe" "$audioToolsPackage/LICENSE" "$audioToolsPackage/README.txt"
if ($LASTEXITCODE -ne 0) { throw 'FFmpeg extraction failed.' }
$audioToolsExtracted = Join-Path $audioToolsWork "$audioToolsPackage\bin\ffmpeg.exe"
if ((Get-FileHash -LiteralPath $audioToolsExtracted -Algorithm SHA256).Hash.ToLowerInvariant() -ne $audioToolsExecutableHash) { throw 'FFmpeg runtime failed SHA256 verification.' }
Copy-Item -LiteralPath $audioToolsExtracted -Destination $audioToolsExecutable -Force
Copy-Item -LiteralPath (Join-Path $audioToolsWork "$audioToolsPackage\LICENSE") -Destination (Join-Path $audioToolsDestination 'LICENSE') -Force
Copy-Item -LiteralPath (Join-Path $audioToolsWork "$audioToolsPackage\README.txt") -Destination (Join-Path $audioToolsDestination 'BUILD.txt') -Force
# The checked directory is exclusively created by this invocation.
$audioToolsResolvedWork = [IO.Path]::GetFullPath($audioToolsWork)
$audioToolsResolvedTemp = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
if (-not $audioToolsResolvedWork.StartsWith($audioToolsResolvedTemp, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected temporary path.' }
Remove-Item -LiteralPath $audioToolsResolvedWork -Recurse -Force
Write-Host "Verified FFmpeg is ready: $audioToolsExecutable"

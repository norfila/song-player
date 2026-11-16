param([Parameter(ValueFromRemainingArguments=$true)][string[]]$TauriArgs)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'toolchain.ps1')
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Rust is missing. Install Rust/MSVC as described in README.md.' }
    & npm.cmd exec -- tauri @TauriArgs
    exit $LASTEXITCODE
} finally { Pop-Location }


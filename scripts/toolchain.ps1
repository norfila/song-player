# Dot-source this file to use the optional project-local build toolchain.
$taskProjectRoot = Split-Path -Parent $PSScriptRoot
$taskLocalCargo = Join-Path $taskProjectRoot '.tools\cargo'
if (Test-Path -LiteralPath (Join-Path $taskLocalCargo 'bin\cargo.exe')) {
    $env:CARGO_HOME = $taskLocalCargo
    $env:RUSTUP_HOME = Join-Path $taskProjectRoot '.tools\rustup'
    $env:PATH = (Join-Path $taskLocalCargo 'bin') + ';' + $env:PATH
}
$taskMsvcRoot = Join-Path $taskProjectRoot '.tools\msvc'
if (Test-Path -LiteralPath $taskMsvcRoot) {
    $taskVc = Get-ChildItem -LiteralPath (Join-Path $taskMsvcRoot 'VC\Tools\MSVC') -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $taskSdkRoot = Join-Path $taskMsvcRoot 'Windows Kits\10'
    $taskSdk = Get-ChildItem -LiteralPath (Join-Path $taskSdkRoot 'Include') -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $env:VSCMD_ARG_HOST_ARCH = 'x64'
    $env:VSCMD_ARG_TGT_ARCH = 'x64'
    $env:VCToolsInstallDir = $taskVc.FullName + '\'
    $env:VCToolsVersion = $taskVc.Name
    $env:WindowsSdkDir = $taskSdkRoot + '\'
    $env:WindowsSDKVersion = $taskSdk.Name + '\'
    $env:PATH = (Join-Path $taskVc.FullName 'bin\Hostx64\x64') + ';' + (Join-Path $taskSdkRoot ('bin\' + $taskSdk.Name + '\x64')) + ';' + $env:PATH
    $env:INCLUDE = (Join-Path $taskVc.FullName 'include') + ';' + (Join-Path $taskSdk.FullName 'ucrt') + ';' + (Join-Path $taskSdk.FullName 'shared') + ';' + (Join-Path $taskSdk.FullName 'um') + ';' + (Join-Path $taskSdk.FullName 'winrt')
    $env:LIB = (Join-Path $taskVc.FullName 'lib\x64') + ';' + (Join-Path $taskSdkRoot ('Lib\' + $taskSdk.Name + '\ucrt\x64')) + ';' + (Join-Path $taskSdkRoot ('Lib\' + $taskSdk.Name + '\um\x64'))
}


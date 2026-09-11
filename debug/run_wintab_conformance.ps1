# Build and exercise isolated WinTab providers without touching live pen input.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$keys = @('CARGO_TARGET_DIR','CTL460_TEST_STREAM_NAME','CTL460_TEST_DLL32','CTL460_TEST_DLL64','CTL460_TEST_PEER32')
$prior = @{}
foreach ($key in $keys) { $prior[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
function Build-Step([string[]] $Arguments) {
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) { throw "WinTab conformance step failed: cargo $Arguments" }
}
Push-Location $root
try {
    $env:CARGO_TARGET_DIR = Join-Path $root 'wintab\target\conformance'
    $env:CTL460_TEST_STREAM_NAME = 'Local\CTL460Conformance-' + [Guid]::NewGuid().ToString('N')
    $env:CTL460_TEST_DLL64 = Join-Path $env:CARGO_TARGET_DIR 'release\wintab32.dll'
    $env:CTL460_TEST_DLL32 = Join-Path $env:CARGO_TARGET_DIR 'i686-pc-windows-msvc\release\wintab32.dll'
    Build-Step @('build','--locked','--release','--manifest-path','wintab/Cargo.toml')
    Build-Step @('build','--locked','--release','--manifest-path','wintab/Cargo.toml','--target','i686-pc-windows-msvc')
    Build-Step @('test','--locked','--manifest-path','wintab/Cargo.toml','--test','wintab_tests','--target','i686-pc-windows-msvc','--','--ignored','--nocapture')
    $env:CTL460_TEST_PEER32 = (Get-ChildItem (Join-Path $env:CARGO_TARGET_DIR 'i686-pc-windows-msvc\debug\deps') -Filter 'wintab_tests-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
    Build-Step @('test','--locked','--manifest-path','wintab/Cargo.toml','--test','wintab_tests','--','--ignored','--nocapture')
    Write-Host 'WinTab x86, x64, and cross-architecture DLL conformance passed.'
} finally {
    foreach ($key in $keys) { [Environment]::SetEnvironmentVariable($key, $prior[$key], 'Process') }
    Pop-Location
}

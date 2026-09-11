# Builds and validates the x64 kernel package using the installed WDK and compatible libclang.
# LIBCLANG_PATH must point to libclang 17; signing is a separate release step, never simulated.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $env:LIBCLANG_PATH -or -not (Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH 'libclang.dll'))) {throw 'Set LIBCLANG_PATH to a compatible libclang 17 directory. See docs/BUILDING.md.'}
$kitRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10'
$catalogTool = Get-ChildItem -LiteralPath (Join-Path $kitRoot 'bin') -Recurse -Filter Inf2Cat.exe | Sort-Object FullName -Descending | Select-Object -First 1
$verifyTool = Get-ChildItem -LiteralPath (Join-Path $kitRoot 'Tools') -Recurse -Filter infverif.exe | Where-Object FullName -Match '\\x64\\' | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $catalogTool -or -not $verifyTool) {throw 'Install the Windows Driver Kit (WDK) with Inf2Cat and InfVerif.'}
$previousTarget = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = Join-Path $projectRoot 'kernel\target'
    Push-Location (Join-Path $projectRoot 'kernel')
    try {
        & cargo fmt -- --check
        if ($LASTEXITCODE -ne 0) {throw 'Kernel formatting check failed'}
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) {throw 'Kernel build failed'}
    } finally {Pop-Location}
    $package = Join-Path $projectRoot 'dist\kernel'
    New-Item -ItemType Directory -Force -Path $package | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'kernel\target\release\ctl460_vhf.dll') -Destination (Join-Path $package 'ctl460_vhf.sys') -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'kernel\ctl460_vhf.inf') -Destination $package -Force
    & $verifyTool.FullName /w (Join-Path $package 'ctl460_vhf.inf')
    if ($LASTEXITCODE -ne 0) {throw 'INF verification failed'}
    & $catalogTool.FullName ('/driver:' + $package) /os:10_X64 /uselocaltime
    if ($LASTEXITCODE -ne 0) {throw 'Catalog generation failed'}
    Write-Output 'Built unsigned kernel package in dist/kernel. Microsoft production signing is required for normal Windows 11 installation.'
} finally {$env:CARGO_TARGET_DIR = $previousTarget}

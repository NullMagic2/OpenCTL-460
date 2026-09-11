# Removes only target/ and debug/generated/ inside this project; -WhatIf previews.
# Source tests, profiles, the lockfile, dist/, and files outside this project survive.
[CmdletBinding(SupportsShouldProcess = $true)]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd('\')
if (-not (Test-Path -LiteralPath (Join-Path $projectRoot 'Cargo.toml'))) { throw 'Project manifest missing; refusing cleanup.' }
$prefix = $projectRoot + '\'
$targets = @('target', 'wintab\target', 'kernel\target', 'debug\generated') | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $projectRoot $_)) }
# Validate every target before deleting anything; never cross directory links/junctions.
foreach ($target in $targets) {
    if (-not $target.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw "Path outside project: $target" }
    $ancestor = $target
    while ($ancestor -and $ancestor.StartsWith($projectRoot, [StringComparison]::OrdinalIgnoreCase)) {
        if (Test-Path -LiteralPath $ancestor) {
            if ((Get-Item -Force -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Refusing linked path: $ancestor" }
        }
        $ancestor = Split-Path -Parent $ancestor
    }
    if (Test-Path -LiteralPath $target) {
        $links = Get-ChildItem -Force -Recurse -LiteralPath $target | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }
        if ($links) { throw "Refusing cleanup through links under $target" }
    }
}
foreach ($target in $targets) {
    if ((Test-Path -LiteralPath $target) -and $PSCmdlet.ShouldProcess($target, 'Remove temporary build files')) {
        Remove-Item -LiteralPath $target -Force -Recurse
    }
}

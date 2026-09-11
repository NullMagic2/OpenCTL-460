# Collects dependency license notices from Cargo's local registry for binary redistribution.
# Each generated text file starts with its purpose and preserves the upstream license text.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $packages = @()
    foreach ($manifest in @('Cargo.toml','wintab\Cargo.toml','kernel\Cargo.toml')) {
        $json = & cargo metadata --manifest-path $manifest --filter-platform x86_64-pc-windows-msvc --locked --offline --format-version 1
        if ($LASTEXITCODE -ne 0) { throw "Cargo metadata failed: $manifest" }
        $metadata = $json | ConvertFrom-Json
        $resolved = @($metadata.resolve.nodes.id)
        $packages += $metadata.packages | Where-Object { $_.id -in $resolved }
    }
    $destination = Join-Path $projectRoot 'third-party-licenses'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    foreach ($package in ($packages | Sort-Object id -Unique)) {
        if (-not $package.source) { continue }
        $packageRoot = Split-Path -Parent $package.manifest_path
        $files = Get-ChildItem -LiteralPath $packageRoot -File -Recurse | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([.-]|$)' }
        if (-not $files -and $package.repository -eq 'https://github.com/microsoft/windows-drivers-rs' -and $package.license -match 'MIT') {
            $files = @(Get-Item -LiteralPath (Join-Path $projectRoot 'licenses\windows-drivers-rs-MIT.txt'))
        }
        $parts = @("Dependency license notices for $($package.name) $($package.version).", "Declared license: $($package.license)", '')
        foreach ($file in $files) {
            $relative = $file.Name
            $parts += "--- Upstream file: $relative ---"
            $parts += Get-Content -Raw -LiteralPath $file.FullName
        }
        if (-not $files) { throw "No license notice found for $($package.name)" }
        $parts -join "`r`n" | Set-Content -Encoding UTF8 -LiteralPath (Join-Path $destination "$($package.name)-$($package.version).txt")
    }
} finally { Pop-Location }

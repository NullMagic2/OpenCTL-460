# Copies the setup supervisor and records hashes of the bundled development package; never installs it.
$ErrorActionPreference='Stop'
$env:PSModulePath=(Join-Path $PSHOME 'Modules')+[IO.Path]::PathSeparator+$env:PSModulePath
$root=Split-Path -Parent $PSScriptRoot
foreach($base in @('dist','target\release')) {
    $assets=Join-Path $root ($base+'\assets')
    New-Item -ItemType Directory -Path $assets -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'assets\bamboo-pen.png'),(Join-Path $root 'assets\bamboo-tablet.png'),(Join-Path $root 'assets\README.md') -Destination $assets -Force
}
$destination=Join-Path $root 'dist\scripts'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'hid_session.ps1'),(Join-Path $PSScriptRoot 'hid_setup_common.ps1') -Destination $destination -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'install_service.ps1'),(Join-Path $PSScriptRoot 'install_virtual_hid.ps1') -Destination $destination -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'startup_shortcut.ps1'),(Join-Path $PSScriptRoot 'system_setup.ps1') -Destination $destination -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'check_tablet_apps.ps1') -Destination $destination -Force
$package=Join-Path $root 'dist\kernel-test'
if(Test-Path -LiteralPath (Join-Path $package 'CTL460-Test.cer')) {
    $certificate=[Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $package 'CTL460-Test.cer'))
    try {
        $lines=@('# Integrity metadata for the bundled CTL-460 development package.','@{',("Thumbprint = '"+$certificate.Thumbprint+"'"),'Hashes = @{')
        foreach($name in @('ctl460_vhf.sys','ctl460_vhf.cat','ctl460_vhf.inf','CTL460-Test.cer')) {
            $hash=(Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash
            $lines+="'$name' = '$hash'"
        }
        $lines+=@('}','}')
        [IO.File]::WriteAllLines((Join-Path $package 'package-hashes.psd1'),$lines)
    } finally {$certificate.Dispose()}
}

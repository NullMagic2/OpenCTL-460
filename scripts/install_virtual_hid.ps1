# Installer-only kernel package repair. Uses the installer's existing elevation; never changes boot policy.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'hid_setup_common.ps1')
$root=Split-Path -Parent $PSScriptRoot
$package=Join-Path $root 'kernel-test'
$thumbprint=Test-HidPackage $package
$certificate=[Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $package 'CTL460-Test.cer'))
$added=[Collections.Generic.List[string]]::new();$installed=$false
try {
    foreach($name in @('Root','TrustedPublisher')) {
        $store=[Security.Cryptography.X509Certificates.X509Store]::new($name,'LocalMachine')
        try {
            $store.Open('ReadWrite')
            if(-not $store.Certificates.Find('FindByThumbprint',$thumbprint,$false).Count){$store.Add($certificate);$added.Add($name)}
        } finally {$store.Dispose()}
    }
    foreach($name in @('ctl460_vhf.sys','ctl460_vhf.cat')) {
        if((Get-AuthenticodeSignature -LiteralPath (Join-Path $package $name)).Status.ToString() -ne 'Valid'){throw "Windows signature verification failed: $name"}
    }
    & (Join-Path $root 'ctl460-setup.exe') install (Join-Path $package 'ctl460_vhf.inf')
    $result=$LASTEXITCODE
    if($result -notin @(0,3010)){throw 'Virtual HID installation failed. Check the signing requirements in docs\SIGNING.md.'}
    $installed=$true
} finally {
    if(-not $installed){foreach($name in $added){$store=[Security.Cryptography.X509Certificates.X509Store]::new($name,'LocalMachine');try{$store.Open('ReadWrite');$store.Remove($certificate)}finally{$store.Dispose()}}}
    $certificate.Dispose()
}
exit $result

# Shared setup decisions, command quoting and reversible Wacom service handoff; safe to dot-source in tests.
# A batch launch from PowerShell 7 can inherit a module path missing Windows PowerShell's built-ins.
$env:PSModulePath=(Join-Path $PSHOME 'Modules')+[IO.Path]::PathSeparator+$env:PSModulePath
function Get-HidSetupAction {
    param([int]$DeviceError,[bool]$TestMode,[bool]$SecureBoot,[bool]$PackagePresent)
    if ($DeviceError -eq 0) {return 'ready'}
    if ($DeviceError -eq 32) {return 'busy'}
    if ($DeviceError -eq 5) {return 'denied'}
    if ($DeviceError -notin @(2,3)) {return 'device_error'}
    if (-not $PackagePresent) {return 'package_missing'}
    if (-not $TestMode -and $SecureBoot) {return 'secure_boot'}
    if (-not $TestMode) {return 'enable_test_mode'}
    return 'install'
}
function Quote-HidArgument {
    param([string]$Value)
    # Windows command-line escaping: preserve trailing slashes and embedded quotes as data.
    return '"' + (($Value -replace '(\\*)"','$1$1\"') -replace '(\\+)$','$1$1') + '"'
}
function Invoke-WacomHandoff {
    param([scriptblock]$Body,[scriptblock]$IsRunning,[scriptblock]$StopService,[scriptblock]$StartService)
    $paused=[Collections.Generic.List[string]]::new()
    try {
        foreach($name in @('TouchServicePen','TabletServicePen')) {
            if (& $IsRunning $name) {
                # Record before stopping: a timeout can occur after the stop request succeeded.
                $paused.Add($name)
                & $StopService $name
            }
        }
        & $Body
    } finally {
        $errors=[Collections.Generic.List[string]]::new()
        for($i=$paused.Count-1;$i -ge 0;$i--) {
            try { & $StartService $paused[$i] } catch {$errors.Add("$($paused[$i]): $_")}
        }
        if($errors.Count) {throw ('Could not restore Wacom services: '+($errors -join '; '))}
    }
}
function Test-HidPackage {
    param([string]$Directory)
    $manifest=Import-PowerShellDataFile -LiteralPath (Join-Path $Directory 'package-hashes.psd1')
    foreach($name in @('ctl460_vhf.sys','ctl460_vhf.cat','ctl460_vhf.inf','CTL460-Test.cer')) {
        $expected=$manifest.Hashes[$name]
        if($expected -notmatch '^[0-9A-Fa-f]{64}$' -or (Get-FileHash -LiteralPath (Join-Path $Directory $name) -Algorithm SHA256).Hash -ne $expected) {
            throw "Virtual HID package is incomplete or changed ($name). Reinstall CTL-460 Studio."
        }
    }
    $certificate=[Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $Directory 'CTL460-Test.cer'))
    try {
        if($certificate.Thumbprint -ne $manifest.Thumbprint -or $certificate.NotAfter -lt [DateTime]::Now -or $certificate.NotBefore -gt [DateTime]::Now) {throw 'Development certificate does not match this package or has expired.'}
        foreach($name in @('ctl460_vhf.sys','ctl460_vhf.cat')) {
            $signature=Get-AuthenticodeSignature -LiteralPath (Join-Path $Directory $name)
            if($signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint -or $signature.Status.ToString() -in @('NotSigned','HashMismatch','NotSupportedFileFormat')) {throw "Invalid development signature: $name"}
        }
        return $certificate.Thumbprint
    } finally {$certificate.Dispose()}
}

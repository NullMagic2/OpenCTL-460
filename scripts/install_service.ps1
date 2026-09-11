# Installer-only registration/removal of the fixed, protected Virtual HID service executable.
param([ValidateSet('Install','Stop','Remove','Inspect')][string]$Action='Install')
$ErrorActionPreference='Stop'
$env:PSModulePath=(Join-Path $PSHOME 'Modules')+[IO.Path]::PathSeparator+$env:PSModulePath
$name='OpenCTL460Broker'
$binary=Join-Path ([Environment]::GetFolderPath('CommonProgramFiles')) 'OpenCTL460\ctl460-service.exe'
$expected='"'+$binary+'"'
# Read-only packaging check: report the same path used to register the service.
if($Action -eq 'Inspect'){Write-Output $binary;exit 0}
$service=Get-Service -Name $name -ErrorAction SilentlyContinue
if($service) {
    $registered=(Get-ItemProperty -LiteralPath ('HKLM:\SYSTEM\CurrentControlSet\Services\'+$name)).ImagePath
    if($registered -ne $expected){throw 'Existing service points to an unexpected binary; setup will not modify it.'}
    if($service.Status -ne 'Stopped'){
        Stop-Service -Name $name -ErrorAction Stop
        $service.WaitForStatus('Stopped',[TimeSpan]::FromSeconds(30))
    }
}
if($Action -eq 'Stop'){exit 0}
if($Action -eq 'Remove') {
    if($service){& (Join-Path $env:SystemRoot 'System32\sc.exe') delete $name | Out-Null;if($LASTEXITCODE -ne 0){throw 'Cannot remove OpenCTL 460 service'}}
    exit 0
}
if(-not (Test-Path -LiteralPath $binary -PathType Leaf)){throw "Service executable is missing: $binary"}
if(-not $service){New-Service -Name $name -BinaryPathName $expected -StartupType Automatic -DisplayName 'OpenCTL 460 Virtual HID' -Description 'Relays validated pen reports from the active console user to the OpenCTL virtual tablet.' | Out-Null}
else {Set-Service -Name $name -StartupType Automatic}
Start-Service -Name $name -ErrorAction Stop
(Get-Service -Name $name).WaitForStatus('Running',[TimeSpan]::FromSeconds(10))

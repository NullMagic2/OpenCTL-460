# Runs a non-installing Inno probe to compare its real destination expansion with the service helper.
# No certificate, service, registry or driver changes are made; outputs stay under debug/generated.
param([string]$Compiler='ISCC.exe')
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$source=Get-Content -LiteralPath (Join-Path $root 'installer\ctl460.iss') -Raw
$entry=[regex]::Match($source,'Source: "\.\.\\dist\\ctl460-service\.exe"; DestDir: "([^"]+)"')
if(-not $entry.Success -or $entry.Groups[1].Value -ne '{app}\system-package'){throw 'Service payload must be staged with the app before elevation'}
$folder=Join-Path $PSScriptRoot ('generated\service-package-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$helper=Join-Path $root 'scripts\install_service.ps1'
$probe=@'
; Read-only test of the actual Inno folder constant and PowerShell service registration path.
[Setup]
AppName=OpenCTL service path probe
AppVersion=1
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
OutputDir=.
OutputBaseFilename=service-path-probe
[Code]
function InitializeSetup: Boolean;
var Expected, Actual: String; Code: Integer; Output: TExecOutput;
begin
  Expected := ExpandConstant('@DEST@\ctl460-service.exe');
  if not ExecAndCaptureOutput(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -ExecutionPolicy Bypass -File "@HELPER@" -Action Inspect',
    '', SW_HIDE, ewWaitUntilTerminated, Code, Output) then RaiseException('Cannot run inspection helper');
  if (Code <> 0) or (GetArrayLength(Output.StdOut) <> 1) then RaiseException('Inspection helper failed');
  Actual := Trim(Output.StdOut[0]);
  SaveStringToFile(ExpandConstant('{src}\paths.txt'), Expected + #13#10 + Actual, False);
  { Returning False exits before the wizard and installation. }
  Result := False;
end;
'@
$probe=$probe.Replace('@DEST@','{commoncf64}\OpenCTL460').Replace('@HELPER@',$helper.Replace("'","''"))
$scriptPath=Join-Path $folder 'service-path-probe.iss'
[IO.File]::WriteAllText($scriptPath,$probe)
& $Compiler /Q $scriptPath
if($LASTEXITCODE -ne 0){throw 'Cannot compile service path probe'}
$p=Start-Process -FilePath (Join-Path $folder 'service-path-probe.exe') -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -WindowStyle Hidden -PassThru
try {
    $null=$p.Handle
    if(-not $p.WaitForExit(15000)){$p.Kill();throw 'Read-only service path probe stalled'}
} finally {$p.Dispose()}
$paths=Get-Content -LiteralPath (Join-Path $folder 'paths.txt')
if($paths.Count -ne 2 -or $paths[0] -ne $paths[1]){throw ('Installer/service path mismatch: '+($paths -join ' versus '))}
Write-Output ('PASS: Inno destination and native PowerShell service path agree: '+$paths[0])

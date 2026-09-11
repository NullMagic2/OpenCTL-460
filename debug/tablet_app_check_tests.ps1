# Tests real Restart Manager handles and the wizard's actual preflight function.
# Only fixture files are locked; the Inno probe exits before showing/installing anything.
param([Parameter(Mandatory=$true)][string]$Compiler)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$helper=Join-Path $root 'scripts\check_tablet_apps.ps1'
. $helper
$fixture=Join-Path $PSScriptRoot ('generated\tablet-apps-'+[Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($fixture)|Out-Null
$files=@((Join-Path $fixture 'x64 Wintab32.dll'),(Join-Path $fixture 'x86 Wintab32.dll'))
foreach($file in $files){Set-Content -LiteralPath $file -Value 'fixture'}
if(@(Get-TabletApplications $files).Count){throw 'Unexpected users of unlocked fixtures'}
if(@(Get-TabletApplications @((Join-Path $fixture 'missing.dll'))).Count){throw 'Missing DLL should not block'}
$ps=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
# Use the production entry point and a wrapper pointing at the same fixture list.
$wrapper=Join-Path $fixture 'check.ps1'
$wrapperText="& '$($helper.Replace("'","''"))' -Paths @('$($files[0].Replace("'","''"))','$($files[1].Replace("'","''"))')`r`nexit `$LASTEXITCODE"
[IO.File]::WriteAllText($wrapper,$wrapperText)
$source=Get-Content -Raw -LiteralPath (Join-Path $root 'installer\ctl460.iss')
$capture=[regex]::Match($source,'(?s)procedure CaptureSetupOutput\(.*?\r?\nend;').Value
$check=[regex]::Match($source,'(?s)function TabletApplicationsError\(.*?\r?\nend;').Value
if(-not $capture -or -not $check){throw 'Missing wizard preflight functions'}
$probe=@'
; Non-installing test of the production preflight and its captured app names.
[Setup]
AppName=OpenCTL tablet app check probe
AppVersion=1
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
OutputDir=.
OutputBaseFilename=preflight-probe
[Code]
var SetupDetails: String;
@CAPTURE@
@CHECK@
function InitializeSetup: Boolean;
var Detail: String;
begin
  Detail := TabletApplicationsError('@HELPER@');
  SaveStringToFile(ExpandConstant('{src}\result.txt'), Detail, False);
  Result := False;
end;
'@
$probe=$probe.Replace('@CAPTURE@',$capture).Replace('@CHECK@',$check).Replace('@HELPER@',$wrapper.Replace("'","''"))
$iss=Join-Path $fixture 'preflight-probe.iss'
[IO.File]::WriteAllText($iss,$probe)
& $Compiler /Q $iss
if($LASTEXITCODE -ne 0){throw 'Preflight probe compilation failed'}
function RunProbe {
    $process=Start-Process -FilePath (Join-Path $fixture 'preflight-probe.exe') -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -WindowStyle Hidden -PassThru
    try {
        if(-not $process.WaitForExit(30000)){throw 'Preflight probe timed out'}
    } finally {$process.Dispose()}
    Get-Content -Raw -LiteralPath (Join-Path $fixture 'result.txt')
}
foreach($file in $files){
    $hold=[IO.File]::Open($file,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try {
        $users=@(Get-TabletApplications $files)
        if(-not ($users -match "PID $PID\)")){throw 'Did not name the fixture process'}
        try {Assert-TabletApplicationsClosed $files; throw 'Allowed an active file user'}
        catch {if($_.Exception.Data['OpenCTLExitCode'] -ne 32){throw}}
        $ErrorActionPreference='Continue'
        & $ps -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $wrapper *> (Join-Path $fixture 'helper-output.txt')
        $code=$LASTEXITCODE
        $ErrorActionPreference='Stop'
        if($code -ne 32){throw "Wrong helper exit code: $code"}
        $detail=RunProbe
        if($detail -notmatch "PID $PID\)" -or $detail -notmatch 'Save your work'){throw "Wizard lost the app name/instructions: $detail"}
    } finally {$hold.Dispose()}
    if(@(Get-TabletApplications $files).Count){throw 'Retry retained stale process information'}
    if(RunProbe){throw 'Wizard blocked after the handle was released'}
}
# A failed query must never be treated as an empty application list.
function Get-TabletApplications {throw 'Simulated Restart Manager failure'}
try {Assert-TabletApplicationsClosed $files; throw 'Failed open'}
catch {if($_.Exception.Message -ne 'Simulated Restart Manager failure'){throw}}
Write-Output 'PASS: x64/x86 file users named, code 32, wizard captures names, retry clears, missing files pass, query failures block. No installation performed.'

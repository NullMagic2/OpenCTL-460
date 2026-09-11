# Exercises setup decisions and service restoration with fake services; no UAC or machine changes.
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
. (Join-Path $root 'scripts\hid_setup_common.ps1')
function Assert-Equal($Actual,$Expected) {if($Actual -ne $Expected){throw "Expected '$Expected', got '$Actual'"}}
foreach($case in @(
    @(0,$false,$true,$false,'ready'),@(32,$true,$false,$true,'busy'),@(5,$true,$false,$true,'denied'),
    @(2,$true,$false,$false,'package_missing'),@(2,$false,$true,$true,'secure_boot'),
    @(2,$false,$false,$true,'enable_test_mode'),@(2,$true,$false,$true,'install'),@(123,$true,$false,$true,'device_error')
)) {Assert-Equal (Get-HidSetupAction $case[0] $case[1] $case[2] $case[3]) $case[4]}
Assert-Equal (Quote-HidArgument 'C:\a b\') '"C:\a b\\"'
Assert-Equal (Quote-HidArgument 'a"b') '"a\"b"'
foreach($failAt in @('none','body','stop')) {
    $script:calls=[Collections.Generic.List[string]]::new()
    $caught=$false
    try {
        Invoke-WacomHandoff -IsRunning {param($n) $n -eq 'TabletServicePen'} `
          -StopService {param($n) $script:calls.Add("stop:$n");if($failAt -eq 'stop'){throw 'stop timed out'}} `
          -StartService {param($n) $script:calls.Add("start:$n")} `
          -Body {$script:calls.Add('body');if($failAt -eq 'body'){throw 'feeder failed'}}
    } catch {$caught=$true;if($failAt -eq 'none'){throw}}
    Assert-Equal $caught ($failAt -ne 'none')
    Assert-Equal $script:calls[0] 'stop:TabletServicePen'
    Assert-Equal $script:calls[$script:calls.Count-1] 'start:TabletServicePen'
    if($script:calls.Contains('start:TouchServicePen')){throw 'Originally stopped service was started'}
}
# Restore every paused service even if restoring one fails.
$script:calls=[Collections.Generic.List[string]]::new();$failed=$false
try {
    Invoke-WacomHandoff -IsRunning {$true} -StopService {} -Body {} -StartService {param($n) $script:calls.Add($n);if($n -eq 'TabletServicePen'){throw 'restore failed'}}
} catch {$failed=$true}
Assert-Equal $failed $true
Assert-Equal $script:calls.Count 2
# Parse the real setup scripts as PowerShell, without executing privileged actions.
foreach($name in @('hid_session.ps1','hid_setup_common.ps1','package_hid_setup.ps1','install_service.ps1','install_virtual_hid.ps1')) {
    $tokens=$null;$errors=$null
    $null=[Management.Automation.Language.Parser]::ParseFile((Join-Path $root ('scripts\'+$name)),[ref]$tokens,[ref]$errors)
    if($errors.Count){throw ($errors | Out-String)}
}
# Validate the real package hashes and matching signer as a read-only check when available.
$package=Join-Path $root 'dist\kernel-test'
if(Test-Path -LiteralPath (Join-Path $package 'package-hashes.psd1')) {
    $null=Test-HidPackage $package
    $bad=Join-Path $PSScriptRoot 'generated\bad-hid-package'
    New-Item -ItemType Directory -Path $bad -Force | Out-Null
    foreach($name in @('ctl460_vhf.sys','ctl460_vhf.cat','ctl460_vhf.inf','CTL460-Test.cer','package-hashes.psd1')) {Copy-Item -LiteralPath (Join-Path $package $name) -Destination $bad -Force}
    Add-Content -LiteralPath (Join-Path $bad 'ctl460_vhf.inf') -Value '; Intentional debug corruption'
    $rejected=$false;try {$null=Test-HidPackage $bad} catch {$rejected=$true};Assert-Equal $rejected $true
}
# Run the real supervisor's early-cancel path: the signaled stop event must prevent UAC and setup.
$generated=Join-Path $PSScriptRoot 'generated';New-Item -ItemType Directory -Path $generated -Force | Out-Null
$name='Local\CTL460SetupCancelTest-'+[Guid]::NewGuid().ToString('N')
$event=[Threading.EventWaitHandle]::new($true,[Threading.EventResetMode]::ManualReset,$name)
$status=Join-Path $generated 'cancelled-hid-status.txt'
try {
    $args=@('-NoProfile','-ExecutionPolicy','Bypass','-File',(Quote-HidArgument (Join-Path $root 'scripts\hid_session.ps1')),
        '-ConfigPath','unused','-DevicePath','unused','-StopEvent',$name,'-StatusPath',(Quote-HidArgument $status),'-GuiProcessId',$PID)
    $process=Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList $args -WindowStyle Hidden -PassThru
    $null=$process.Handle
    if(-not $process.WaitForExit(15000)) {$process.Kill();throw 'Cancelled setup did not exit'}
    Assert-Equal $process.ExitCode 0
    Assert-Equal (Get-Content -LiteralPath $status -First 1) 'cancelled'
    $process.Dispose()
} finally {$event.Dispose()}
Write-Output 'PASS: setup states, Windows argument quoting, failure cleanup, service restoration and package validation.'

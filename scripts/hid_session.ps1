# Supervises a normal-user feeder. The installed service owns privileged HID access and Wacom handoff.
# No elevation, system configuration, or executable paths supplied by pipe clients are used here.
[CmdletBinding()]
param([string]$ConfigPath,[string]$DevicePath,[string]$StopEvent,[string]$StatusPath,
      [int]$GuiProcessId,[switch]$Legacy,[switch]$InspectOnly,[string]$TracePath)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'hid_setup_common.ps1')
$root=Split-Path -Parent $PSScriptRoot
function Set-SessionStatus([string]$State,[string]$Message) {
    Write-Output "$State`: $Message"
    if($StatusPath) {
        $temporary=$StatusPath+'.'+$PID+'.tmp'
        [IO.File]::WriteAllText($temporary,($State+"`n"+$Message),[Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $temporary -Destination $StatusPath -Force
    }
}
if($InspectOnly) {
    $service=Get-Service -Name OpenCTL460Broker -ErrorAction SilentlyContinue
    @{serviceInstalled=($null -ne $service);serviceStatus=([string]$service.Status)}|ConvertTo-Json -Compress
    exit 0
}
$event=$null;$gui=$null;$feeder=$null
try {
    if(-not $ConfigPath -or -not $DevicePath -or -not $StopEvent -or -not $StatusPath -or $GuiProcessId -le 0){throw 'Missing GUI session parameters.'}
    $event=[Threading.EventWaitHandle]::OpenExisting($StopEvent)
    $gui=[Diagnostics.Process]::GetProcessById($GuiProcessId)
    $null=$gui.Handle
    if($event.WaitOne(0) -or $gui.HasExited){Set-SessionStatus 'cancelled' 'Start cancelled.';exit 0}
    $service=Get-Service -Name OpenCTL460Broker -ErrorAction SilentlyContinue
    if($service -and $service.Status -eq 'StartPending') {
        Set-SessionStatus 'starting' 'Waiting for the Windows tablet service to finish starting...'
        $deadline=[DateTime]::UtcNow.AddSeconds(30)
        while($service.Status -eq 'StartPending' -and [DateTime]::UtcNow -lt $deadline) {
            if($event.WaitOne(150) -or $gui.HasExited){Set-SessionStatus 'cancelled' 'Start cancelled.';exit 0}
            $service.Refresh()
        }
    }
    if(-not $service -or $service.Status -ne 'Running'){throw 'The OpenCTL 460 service is unavailable. Run the latest installer to install or repair it; routine startup needs no administrator prompt.'}
    $sessionLog=Join-Path (Split-Path -Parent $StatusPath) 'hid-feeder.log'
    $sessionOut=Join-Path (Split-Path -Parent $StatusPath) 'hid-feeder-output.log'
    $arguments=@('run','--device-path',(Quote-HidArgument $DevicePath),'--init','--config',(Quote-HidArgument $ConfigPath),'--stop-event',(Quote-HidArgument $StopEvent))
    if($Legacy){$arguments+='--wacom11'}
    if($TracePath){$arguments+=@('--trace',(Quote-HidArgument $TracePath))}
    Set-SessionStatus 'starting' 'Connecting to the Virtual HID service...'
    $feeder=Start-Process -FilePath (Join-Path $root 'ctl460-rust.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardError $sessionLog -RedirectStandardOutput $sessionOut
    $null=$feeder.Handle
    Set-SessionStatus 'running' 'Virtual HID feeder started through the OpenCTL 460 service.'
    while(-not $feeder.WaitForExit(150)){if($event.WaitOne(0) -or $gui.HasExited){[void]$event.Set();break}}
    if(-not $feeder.WaitForExit(6000)){$feeder.Kill();$feeder.WaitForExit();throw 'Feeder did not stop normally; the service releases the pen on disconnect.'}
    if($feeder.ExitCode -ne 0){throw ('Feeder stopped: '+((Get-Content -LiteralPath $sessionLog -Tail 8 -ErrorAction SilentlyContinue)-join ' '))}
    Set-SessionStatus 'stopped' 'Virtual HID stopped. The service restores previously running Wacom services.'
} catch {Set-SessionStatus 'failed' ([string]$_);exit 1}
finally {
    if($event){[void]$event.Set()}
    if($feeder -and -not $feeder.HasExited){if(-not $feeder.WaitForExit(6000)){$feeder.Kill();$feeder.WaitForExit()}}
    if($feeder){$feeder.Dispose()};if($event){$event.Dispose()};if($gui){$gui.Dispose()}
}

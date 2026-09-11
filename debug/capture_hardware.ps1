# Captures real reports for a bounded time without injecting input; optionally pauses competing Wacom services.
# Only services that were running and actually paused are restarted, including on capture failure.
param([ValidateRange(-1,20)][int]$Device=-1,[ValidateRange(1,60)][int]$Seconds=45,[switch]$PauseWacom,[switch]$NoInitialize)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
$generated=Join-Path $PSScriptRoot 'generated'
New-Item -ItemType Directory -Force -Path $generated | Out-Null
Start-Transcript -Path (Join-Path $generated 'hardware-session.log') -Force | Out-Null
$paused=@()
$eventName='Local\CTL460CaptureStop-'+[Guid]::NewGuid().ToString('N')
$event=[Threading.EventWaitHandle]::new($false,[Threading.EventResetMode]::ManualReset,$eventName)
$capture=$null
try {
    if($PauseWacom){
        foreach($name in @('TouchServicePen','TabletServicePen')){
            $service=Get-Service -Name $name -ErrorAction SilentlyContinue
            if($service -and $service.Status -eq 'Running'){
                Stop-Service -Name $name -ErrorAction Stop
                $paused+=$name
                $service.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Stopped,[TimeSpan]::FromSeconds(10))
                Write-Output "Temporarily paused $name"
            }
        }
    }
    # Collection indices can change after reconnecting; prefer the pen's usage by default.
    # Observe an already-running tablet without repeating its mode command when requested.
    $captureArguments=@('capture','--stop-event',$eventName)
    if(-not $NoInitialize){$captureArguments+='--init'}
    if($Device -ge 0){$captureArguments+=@('--device',$Device)}
    $capture=Start-Process -FilePath (Join-Path $projectRoot 'dist\ctl460-rust.exe') -ArgumentList $captureArguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $generated 'isolated-capture.txt') -RedirectStandardError (Join-Path $generated 'isolated-capture-errors.txt')
    Write-Output "Capture started for up to $Seconds seconds (collection -1 means automatic selection)."
    if(-not $capture.WaitForExit($Seconds*1000)){
        [void]$event.Set()
        if(-not $capture.WaitForExit(5000)){$capture.Kill();throw 'Capture did not shut down gracefully'}
    }
    Write-Output "Capture exit: $($capture.ExitCode)"
} finally {
    [void]$event.Set()
    if($capture -and -not $capture.HasExited){if(-not $capture.WaitForExit(5000)){$capture.Kill()}}
    $event.Dispose()
    foreach($name in $paused){Start-Service -Name $name -ErrorAction Continue;Write-Output "Restored $name"}
    Stop-Transcript | Out-Null
}

# Runs the experimental feeder only against our foreground test canvas, then restores Wacom services.
# Does not install any driver, WinTab DLL, certificate or boot setting.
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$generated=Join-Path $PSScriptRoot 'generated'
New-Item -ItemType Directory -Force -Path $generated | Out-Null
Start-Transcript -Path (Join-Path $generated 'live-ink-session.log') -Force | Out-Null
$unique=[Guid]::NewGuid().ToString('N')
$eventName='Local\CTL460LiveTest-'+$unique
$event=[Threading.EventWaitHandle]::new($false,[Threading.EventResetMode]::ManualReset,$eventName)
$ready=Join-Path $generated ('canvas-ready-'+$unique+'.txt')
$report=Join-Path $generated 'live-ink-result.txt'
$profile=Join-Path $generated 'live-ink-profile.toml'
@'
# Controlled physical-tablet writing test: no keyboard or mouse-button shortcuts.
backend = "ink"
handwriting_mode = "on"
button1 = "None"
button2 = "None"
virtual_tilt = false
smoothing_ms = 3.0
interpolation_ms = 2.0
'@ | Set-Content -LiteralPath $profile -Encoding UTF8
$paused=@();$canvas=$null;$feeder=$null
try {
    foreach($name in @('TouchServicePen','TabletServicePen')){
        $service=Get-Service -Name $name -ErrorAction SilentlyContinue
        if($service -and $service.Status -eq 'Running'){
            Stop-Service -Name $name -ErrorAction Stop;$paused+=$name
            $service.WaitForStatus([ServiceProcess.ServiceControllerStatus]::Stopped,[TimeSpan]::FromSeconds(10))
        }
    }
    # The canvas is intentionally visible: the user draws here and can press Escape to stop.
    $canvas=Start-Process -FilePath (Join-Path $root 'target\release\ctl460-input-test.exe') -ArgumentList @($eventName,('"'+$ready+'"'),('"'+$report+'"')) -WindowStyle Normal -PassThru -RedirectStandardError (Join-Path $generated 'canvas-errors.txt')
    $deadline=[DateTime]::UtcNow.AddSeconds(7)
    while(-not (Test-Path -LiteralPath $ready)){
        if($canvas.HasExited -or [DateTime]::UtcNow -gt $deadline){throw 'Canvas did not become foreground; no pen input started.'}
        Start-Sleep -Milliseconds 100
    }
    $feeder=Start-Process -FilePath (Join-Path $root 'dist\ctl460-rust.exe') -ArgumentList @('run','--device','1','--init','--config',('"'+$profile+'"'),'--stop-event',$eventName) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $generated 'live-feeder-output.txt') -RedirectStandardError (Join-Path $generated 'live-feeder-errors.txt')
    Write-Output 'Experimental Ink is running against the test canvas. Draw now; Escape ends it.'
    if(-not $feeder.WaitForExit(46000)){[void]$event.Set();if(-not $feeder.WaitForExit(5000)){$feeder.Kill()}}
} finally {
    [void]$event.Set()
    if($feeder -and -not $feeder.HasExited){if(-not $feeder.WaitForExit(5000)){$feeder.Kill()}}
    if($canvas -and -not $canvas.HasExited){if(-not $canvas.WaitForExit(5000)){$canvas.Kill()}}
    $event.Dispose()
    foreach($name in $paused){Start-Service -Name $name -ErrorAction Continue;Write-Output "Restored $name"}
    Stop-Transcript | Out-Null
}

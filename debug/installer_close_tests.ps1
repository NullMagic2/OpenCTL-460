# Exercises installer shutdown against isolated app copies and dummy stalled processes; never installs drivers.
$ErrorActionPreference='Stop'
$project=Split-Path -Parent $PSScriptRoot
$fixture=Join-Path $PSScriptRoot ('generated\installer-close-'+[Guid]::NewGuid().ToString('N'))
$real=Join-Path $fixture 'actual app';$hung=Join-Path $fixture 'stalled app';$other=Join-Path $fixture 'unrelated app'
foreach($dir in @($real,$hung,$other)){New-Item -ItemType Directory -Path $dir -Force | Out-Null}
$stub=Join-Path $hung 'ctl460-gui.exe'
Add-Type -TypeDefinition 'public class InstallerStalledFixture { public static void Main() { System.Threading.Thread.Sleep(600000); } }' -OutputAssembly $stub -OutputType ConsoleApplication
Copy-Item -LiteralPath $stub -Destination (Join-Path $hung 'ctl460-rust.exe')
Copy-Item -LiteralPath $stub -Destination (Join-Path $other 'ctl460-gui.exe')
Copy-Item -LiteralPath (Join-Path $project 'target\release\ctl460-gui.exe') -Destination $real
$children=[Collections.Generic.List[Diagnostics.Process]]::new()
function Start-Fixture([string]$File,[string[]]$Arguments) {
    $parameters=@{FilePath=$File;WindowStyle='Hidden';PassThru=$true}
    if($Arguments.Count){$parameters.ArgumentList=$Arguments}
    $p=Start-Process @parameters;$null=$p.Handle;$children.Add($p);return $p
}
function Invoke-Close([string]$Directory,[string]$Label) {
    $out=Join-Path $fixture ($Label+'.txt');$err=Join-Path $fixture ($Label+'-error.txt')
    $args=@('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+(Join-Path $project 'installer\close_application.ps1')+'"'),'-InstallDir',('"'+$Directory+'\."'),'-GraceSeconds','1')
    $p=Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe') -ArgumentList $args -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    try {
        $null=$p.Handle
        if(-not $p.WaitForExit(15000)){$p.Kill();throw 'Installer close helper stalled'}
        if($p.ExitCode -ne 0){throw (Get-Content -LiteralPath $err -Raw)}
        return Get-Content -LiteralPath $out -Raw
    } finally {$p.Dispose()}
}
try {
    $unrelated=Start-Fixture (Join-Path $other 'ctl460-gui.exe') @()
    $gui=Start-Fixture (Join-Path $real 'ctl460-gui.exe') @('--settings-dir',('"'+(Join-Path $real 'settings')+'"'))
    if(-not $gui.WaitForInputIdle(10000)){throw 'Test GUI did not initialize'}
    $log=Invoke-Close $real 'graceful'
    if(-not $gui.HasExited -or $log -notmatch 'Closed ctl460-gui' -or $log -match 'Terminating'){throw 'Hidden GUI did not exit gracefully'}
    if($unrelated.HasExited){throw 'Helper terminated an app outside the installation folder'}
    $stalled=Start-Fixture $stub @()
    $feeder=Start-Fixture (Join-Path $hung 'ctl460-rust.exe') @()
    $log=Invoke-Close $hung 'forced'
    if(-not $stalled.HasExited -or -not $feeder.HasExited){throw 'Stalled app or orphan feeder remains running'}
    if($log -notmatch 'Terminating unresponsive ctl460-gui' -or $log -notmatch 'Terminating unresponsive ctl460-rust'){throw 'Force-exit fallback was not exercised'}
    if($unrelated.HasExited){throw 'Fallback terminated an unrelated app copy'}
    $null=Invoke-Close $hung 'already-stopped'
    Write-Output 'PASS: hidden GUI graceful exit, force-exit fallback, orphan feeder cleanup, exact folder scope and repeated setup.'
} finally {
    foreach($p in $children){try{if(-not $p.HasExited){$p.Kill();$null=$p.WaitForExit(5000)}}finally{$p.Dispose()}}
}

# Elevated installer boundary for the shared driver, broker and WinTab DLLs.
# The application may be per user; the LocalSystem binary must never run from that directory.
param([Parameter(Mandatory=$true)][ValidateSet('Install','Remove')][string]$Action,
      [Parameter(Mandatory=$true)][string]$AppDir)
$ErrorActionPreference='Stop'
$AppDir=[IO.Path]::GetFullPath($AppDir).TrimEnd('\')
$systemRoot=Join-Path ([Environment]::GetFolderPath('CommonProgramFiles')) 'OpenCTL460'
$manifest=Join-Path $systemRoot 'ownership.json'
$powershell=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$log=Join-Path $AppDir 'system-setup.log'
$restartBrokerOnFailure=$false
. (Join-Path $PSScriptRoot 'check_tablet_apps.ps1')
function AssertWritable([string]$path) {
    # Opening for write detects a loaded image or conflicting file handle without
    # truncating or writing the file. Check BOTH architectures before any mutation.
    try {
        $handle=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Write,
            ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
        $handle.Dispose()
    } catch {
        $cause=$_.Exception
        while($cause.InnerException){$cause=$cause.InnerException}
        if(($cause.HResult -band 0xffff) -in @(32,33)){
            $failure=[Exception]::new("WinTab is in use: $path. Save your work and close drawing applications, then retry.")
            $failure.Data['OpenCTLExitCode']=32
            throw $failure
        }
        throw
    }
}
function StopBrokerForInstall {
    $service=Get-Service -Name OpenCTL460Broker -ErrorAction SilentlyContinue
    $script:restartBrokerOnFailure=$service -and $service.Status -eq 'Running'
    $null=RunHelper 'install_service.ps1' @('-Action','Stop')
}
function RestoreBrokerAfterFailure {
    if($restartBrokerOnFailure){
        $binary=(Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Services\OpenCTL460Broker').ImagePath
        if($binary -eq ('"'+(Join-Path $systemRoot 'ctl460-service.exe')+'"')){
            Start-Service -Name OpenCTL460Broker -ErrorAction Stop
        }
    }
}
function RunHelper([string]$name,[string[]]$arguments=@()) {
    & $powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $AppDir ('scripts\'+$name)) @arguments | Out-Host
    if($LASTEXITCODE -notin @(0,3010)){throw "$name failed ($LASTEXITCODE). See system-setup.log."}
    return $LASTEXITCODE
}
function SaveOwnership { $ownership | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifest -Encoding UTF8 }
function ClearRegistration {
    $registered=(Get-ItemProperty -LiteralPath 'HKLM:\Software\OpenCTL460' -ErrorAction SilentlyContinue).InstallPath
    if([string]::Equals($registered,$AppDir,[StringComparison]::OrdinalIgnoreCase)){
        Remove-ItemProperty -LiteralPath 'HKLM:\Software\OpenCTL460' -Name InstallPath -ErrorAction SilentlyContinue
    }
}
function LegacyOriginal([string]$arch) {
    $ini=Join-Path $AppDir 'wintab-backup\ownership.ini'
    if(-not(Test-Path -LiteralPath $ini)){return $null}
    $section=''; $original=''; $installed=''
    foreach($line in Get-Content -LiteralPath $ini){
        if($line -match '^\[(.+)\]'){$section=$Matches[1]}
        elseif($section -eq $arch -and $line -match '^original=(.+)$'){$original=$Matches[1].Trim()}
        elseif($section -eq $arch -and $line -match '^installed=(.+)$'){$installed=$Matches[1].Trim()}
    }
    if($installed -match '^[a-fA-F0-9]{64}$' -and ($original -eq 'none' -or $original -match '^[a-fA-F0-9]{64}$')) {
        return @{ original=$original; installed=$installed }
    }
    return $null
}
Start-Transcript -LiteralPath $log -Append | Out-Null
try {
    if(-not([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){
        throw 'Administrator approval is required for Windows tablet components.'
    }
    # Recheck immediately before changing shared components, including direct
    # helper invocations and apps opened since the wizard's early check.
    Assert-TabletApplicationsClosed @(
        (Join-Path $env:SystemRoot 'System32\Wintab32.dll'),
        (Join-Path $env:SystemRoot 'SysWOW64\Wintab32.dll'))
    $ownership=@{ app=$AppDir; dlls=@{} }
    if($Action -eq 'Remove' -and -not(Test-Path -LiteralPath $manifest)){
        $null=RunHelper 'startup_shortcut.ps1' @('-Action','RemoveAll','-Executable',(Join-Path $AppDir 'ctl460-gui.exe'))
        ClearRegistration
        exit 0
    }
    if(Test-Path -LiteralPath $manifest){
        $stored=Get-Content -Raw -LiteralPath $manifest | ConvertFrom-Json
        if(-not [string]::Equals($stored.app,$AppDir,[StringComparison]::OrdinalIgnoreCase)){
            throw "OpenCTL is already installed at $($stored.app). Uninstall that copy before changing installation scope or folder."
        }
        foreach($p in $stored.dlls.PSObject.Properties){$ownership.dlls[$p.Name]=@{original=$p.Value.original;installed=$p.Value.installed}}
    }
    foreach($arch in @('x64','x86')){
        $folder=if($arch -eq 'x64'){'System32'}else{'SysWOW64'}
        $target=Join-Path $env:SystemRoot ($folder+'\Wintab32.dll')
        if(-not(Test-Path -LiteralPath $target)){continue}
        $current=(Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash
        if($Action -eq 'Install'){
            $incoming=(Get-FileHash -LiteralPath (Join-Path $AppDir ('system-package\'+$arch+'\Wintab32.dll')) -Algorithm SHA256).Hash
            if($incoming -ne $current){AssertWritable $target}
        } elseif($ownership.dlls[$arch] -and $ownership.dlls[$arch].installed -eq $current){
            AssertWritable $target
        }
    }
    if($Action -eq 'Install') {
        [IO.Directory]::CreateDirectory($systemRoot) | Out-Null
        New-Item -Path 'HKLM:\Software\OpenCTL460' -Force | Out-Null
        New-ItemProperty -LiteralPath 'HKLM:\Software\OpenCTL460' -Name InstallPath -Value $AppDir -PropertyType String -Force | Out-Null
        StopBrokerForInstall
        foreach($arch in @('x64','x86')) {
            $folder=if($arch -eq 'x64'){'System32'}else{'SysWOW64'}
            $target=Join-Path $env:SystemRoot ($folder+'\Wintab32.dll')
            $source=Join-Path $AppDir ('system-package\'+$arch+'\Wintab32.dll')
            $incoming=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
            $current=if(Test-Path -LiteralPath $target){(Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash}else{'none'}
            $record=$ownership.dlls[$arch]
            if(-not $record){
                $legacy=LegacyOriginal $arch
                if($legacy -and $current -eq $legacy.installed){
                    $record=$legacy
                    if($record.original -ne 'none'){
                        $backup=Join-Path $AppDir ('wintab-backup\'+$arch+'\original-'+$record.original+'.dll')
                        if((Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash -ne $record.original){throw 'Legacy WinTab backup verification failed.'}
                        Copy-Item -LiteralPath $backup -Destination (Join-Path $systemRoot ($arch+'-'+$record.original+'.dll')) -Force
                    }
                } else {$record=@{original=$current;installed=''}}
            }
            if($record.installed -and $current -ne $record.installed -and $current -ne $incoming){
                # A newer external driver owns the current file: preserve that as the restore point.
                $record.original=$current
            }
            if($record.original -eq $current -and $current -ne 'none'){
                $backup=Join-Path $systemRoot ($arch+'-'+$current+'.dll')
                if(-not(Test-Path -LiteralPath $backup)){Copy-Item -LiteralPath $target -Destination $backup}
                if((Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash -ne $current){throw 'WinTab backup verification failed.'}
            }
            $ownership.dlls[$arch]=$record
            SaveOwnership
            if($current -ne $incoming){Copy-Item -LiteralPath $source -Destination $target -Force}
            if((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $incoming){throw 'Installed WinTab verification failed.'}
            $record.installed=$incoming
            SaveOwnership
        }
        Copy-Item -LiteralPath (Join-Path $AppDir 'system-package\ctl460-service.exe') -Destination (Join-Path $systemRoot 'ctl460-service.exe') -Force
        $reboot=RunHelper 'install_virtual_hid.ps1'
        $null=RunHelper 'install_service.ps1' @('-Action','Install')
        exit $reboot
    }
    # Removal runs before Inno deletes the app. Refuse to remove a different installation.
    $null=RunHelper 'install_service.ps1' @('-Action','Remove')
    & (Join-Path $AppDir 'ctl460-setup.exe') remove
    if($LASTEXITCODE -notin @(0,3010)){throw 'Cannot remove the virtual tablet. Close drawing apps and retry.'}
    foreach($arch in @('x64','x86')){
        $record=$ownership.dlls[$arch]; if(-not $record){continue}
        $folder=if($arch -eq 'x64'){'System32'}else{'SysWOW64'}
        $target=Join-Path $env:SystemRoot ($folder+'\Wintab32.dll')
        if(-not(Test-Path -LiteralPath $target)){continue}
        if((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $record.installed){continue}
        if($record.original -eq 'none'){Remove-Item -LiteralPath $target -Force}
        elseif($record.original -match '^[a-fA-F0-9]{64}$'){
            $backup=Join-Path $systemRoot ($arch+'-'+$record.original+'.dll')
            if((Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash -ne $record.original){throw 'WinTab backup verification failed.'}
            Copy-Item -LiteralPath $backup -Destination $target -Force
        } else {throw 'Invalid WinTab restore record; backup preserved.'}
    }
    $null=RunHelper 'startup_shortcut.ps1' @('-Action','RemoveAll','-Executable',(Join-Path $AppDir 'ctl460-gui.exe'))
    # No recursive deletion: retain original driver backups for recovery.
    foreach($name in @('ownership.json','ctl460-service.exe')){
        $path=Join-Path $systemRoot $name
        if(Test-Path -LiteralPath $path){Remove-Item -LiteralPath $path -Force}
    }
    ClearRegistration
} catch {
    $failure=$_
    # A new app can acquire the DLL between the preflight and the actual copy.
    $cause=$failure.Exception
    while($cause.InnerException){$cause=$cause.InnerException}
    if(($cause.HResult -band 0xffff) -in @(32,33)){
        $failure.Exception.Data['OpenCTLExitCode']=32
    }
    try { RestoreBrokerAfterFailure } catch { [Console]::Error.WriteLine("Could not restore the previous service: $($_.Exception.Message)") }
    [Console]::Error.WriteLine($failure.Exception.Message)
    if($failure.Exception.Data['OpenCTLExitCode']){exit ([int]$failure.Exception.Data['OpenCTLExitCode'])}
    exit 1
}
finally { Stop-Transcript | Out-Null }

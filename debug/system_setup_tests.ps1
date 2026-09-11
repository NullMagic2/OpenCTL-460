# Exercise the installer helper against ordinary fixture files. No service, registry,
# certificate, input, or Windows directory access: privileged boundaries are replaced below.
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$fixture=Join-Path $PSScriptRoot ('generated\system-package-'+[Guid]::NewGuid().ToString('N'))
$app=Join-Path $fixture 'app with spaces'
$common=Join-Path $fixture 'protected'
$windows=Join-Path $fixture 'windows'
foreach($dir in @($app,$common,(Join-Path $windows 'System32'),(Join-Path $windows 'SysWOW64'),(Join-Path $app 'system-package\x64'),(Join-Path $app 'system-package\x86'))){[IO.Directory]::CreateDirectory($dir)|Out-Null}
$source=Get-Content -Raw -LiteralPath (Join-Path $root 'scripts\system_setup.ps1')
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseInput($source,[ref]$tokens,[ref]$errors)
if($errors){throw $errors[0]}
$helper=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'RunHelper'},$true)
if(-not $helper){throw 'Missing helper boundary'}
$source=$source.Replace($helper.Extent.Text,'function RunHelper([string]$name,[string[]]$arguments=@()) { return 0 }')
$registration=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'ClearRegistration'},$true)
if(-not $registration){throw 'Missing registry boundary'}
$source=$source.Replace($registration.Extent.Text,'function ClearRegistration {}')
# Keep service state and Restart Manager outside this filesystem-only fixture.
foreach($name in @('StopBrokerForInstall','RestoreBrokerAfterFailure')){
    $boundary=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $name},$true)
    if(-not $boundary){throw "Missing boundary: $name"}
    $source=$source.Replace($boundary.Extent.Text,('function '+$name+' {}'))
}
$source=$source.Replace(". (Join-Path `$PSScriptRoot 'check_tablet_apps.ps1')",'function Assert-TabletApplicationsClosed([string[]]$ResourcePaths) {}')
$source=$source -replace '(?m)^\$systemRoot=.*$', ('$systemRoot='''+$common.Replace("'","''")+'''')
$source=$source.Replace('$env:SystemRoot',("'"+$windows.Replace("'","''")+"'"))
$admin="([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"
if(-not $source.Contains($admin)){throw 'Missing privilege boundary'}
$source=$source.Replace($admin,'$true')
$source=$source.Replace("& (Join-Path `$AppDir 'ctl460-setup.exe') remove",'$global:LASTEXITCODE=0')
$source=($source -split "`n" | Where-Object {$_ -notmatch '(New-Item|New-ItemProperty|Remove-ItemProperty).*HKLM:'}) -join "`n"
if($source -match 'HKLM:|env:SystemRoot|IsInRole|\& \(Join-Path|\& \$powershell'){throw 'A privileged boundary was not replaced'}
$script=Join-Path $fixture 'fixture-system-setup.ps1'
[IO.File]::WriteAllText($script,$source)
$ps=Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
function Run([string]$action,[string]$directory=$app,[bool]$success=$true) {
    $ErrorActionPreference='Continue'
    & $ps -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $script -Action $action -AppDir $directory *> (Join-Path $fixture 'last-run.txt')
    $ErrorActionPreference='Stop'
    if(($LASTEXITCODE -eq 0) -ne $success){throw (Get-Content -Raw -LiteralPath (Join-Path $fixture 'last-run.txt'))}
}
function Hash([string]$path){(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash}
$legacy=@()
foreach($arch in @('x64','x86')) {
    $folder=if($arch -eq 'x64'){'System32'}else{'SysWOW64'}
    $target=Join-Path $windows ($folder+'\Wintab32.dll')
    Set-Content -LiteralPath $target -Value ('old-openctl-'+$arch)
    $installed=Hash $target
    $vendor=Join-Path $fixture ('vendor-'+$arch+'.dll'); Set-Content -LiteralPath $vendor -Value ('original-vendor-'+$arch)
    $original=Hash $vendor
    $backup=Join-Path $app ('wintab-backup\'+$arch);[IO.Directory]::CreateDirectory($backup)|Out-Null
    Copy-Item -LiteralPath $vendor -Destination (Join-Path $backup ('original-'+$original+'.dll'))
    $legacy+=@("[$arch]","original=$original","installed=$installed")
    Set-Content -LiteralPath (Join-Path $app ('system-package\'+$arch+'\Wintab32.dll')) -Value ('new-openctl-'+$arch)
}
Set-Content -LiteralPath (Join-Path $app 'wintab-backup\ownership.ini') -Value $legacy
Set-Content -LiteralPath (Join-Path $app 'system-package\ctl460-service.exe') -Value 'service-fixture'
# Both architectures must be checked before changing either DLL or ownership.
$blocked=Join-Path $windows 'SysWOW64\Wintab32.dll'
$before64=Hash (Join-Path $windows 'System32\Wintab32.dll')
$hold=[IO.File]::Open($blocked,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try {
    Run 'Install' $app $false
    if($LASTEXITCODE -ne 32){throw 'Locked WinTab did not return retryable code 32'}
    if((Hash (Join-Path $windows 'System32\Wintab32.dll')) -ne $before64){throw 'Changed x64 before detecting x86 lock'}
    if(Test-Path -LiteralPath (Join-Path $common 'ownership.json')){throw 'Wrote ownership before lock check'}
} finally {$hold.Dispose()}
Run 'Install'
$record=Get-Content -Raw -LiteralPath (Join-Path $common 'ownership.json') | ConvertFrom-Json
foreach($arch in @('x64','x86')){
    if($record.dlls.$arch.original -ne (Hash (Join-Path $fixture ('vendor-'+$arch+'.dll')))){throw 'Lost original vendor backup during upgrade'}
    if($record.dlls.$arch.installed -ne (Hash (Join-Path $app ('system-package\'+$arch+'\Wintab32.dll')))){throw 'Wrong installed hash'}
}
# Repair and version upgrade must keep the original restoration point.
Set-Content -LiteralPath (Join-Path $app 'system-package\x64\Wintab32.dll') -Value 'second-openctl-x64'
Run 'Install'
$other=Join-Path $fixture 'other app';[IO.Directory]::CreateDirectory($other)|Out-Null
Run 'Remove' $other $false
if(-not(Test-Path -LiteralPath (Join-Path $common 'ownership.json'))){throw 'Other installation removed ownership'}
# Another vendor replacing one architecture must survive uninstall unchanged.
$foreign=Join-Path $windows 'SysWOW64\Wintab32.dll';Set-Content -LiteralPath $foreign -Value 'foreign-replacement'
$foreignHash=Hash $foreign
Run 'Remove'
if((Hash (Join-Path $windows 'System32\Wintab32.dll')) -ne (Hash (Join-Path $fixture 'vendor-x64.dll'))){throw 'Original vendor DLL was not restored'}
if((Hash $foreign) -ne $foreignHash){throw 'Foreign replacement was modified'}
if(Test-Path -LiteralPath (Join-Path $common 'ownership.json')){throw 'Ownership not cleaned'}
Write-Output 'PASS: locked x86 blocks x64 changes, retry after release, legacy migration, repair, upgrade, ownership isolation, and uninstall restoration (sandbox files only).'

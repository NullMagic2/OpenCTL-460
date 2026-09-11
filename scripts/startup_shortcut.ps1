# Per-user startup link. Uninstall removes only links targeting this installation.
param(
    [Parameter(Mandatory=$true)][ValidateSet('Enable','Disable','RemoveAll')][string]$Action,
    [Parameter(Mandatory=$true)][string]$Executable,
    [string]$StartupDirectory = [Environment]::GetFolderPath('Startup')
)
$ErrorActionPreference='Stop'
$Executable=[IO.Path]::GetFullPath($Executable)
$shell=New-Object -ComObject WScript.Shell
function RemoveOwnedLink([string]$directory) {
    if(-not $directory){return}
    $path=Join-Path $directory 'OpenCTL 460.lnk'
    if(Test-Path -LiteralPath $path -PathType Leaf){
        $link=$shell.CreateShortcut($path)
        if([string]::Equals($link.TargetPath,$Executable,[StringComparison]::OrdinalIgnoreCase)){
            Remove-Item -LiteralPath $path -Force
        }
    }
}
try {
    if($Action -eq 'Enable') {
        if(-not(Test-Path -LiteralPath $Executable -PathType Leaf)){throw 'Application executable is missing.'}
        [IO.Directory]::CreateDirectory($StartupDirectory) | Out-Null
        $link=$shell.CreateShortcut((Join-Path $StartupDirectory 'OpenCTL 460.lnk'))
        $link.TargetPath=$Executable
        $link.Arguments='--startup'
        $link.WorkingDirectory=Split-Path -Parent $Executable
        $link.Description='Start OpenCTL 460 in the system tray'
        $link.IconLocation=$Executable+',0'
        $link.Save()
    } elseif($Action -eq 'Disable') { RemoveOwnedLink $StartupDirectory }
    else {
        RemoveOwnedLink $StartupDirectory
        foreach($profile in Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\*'){
            if($profile.ProfileImagePath){
                $homePath=[Environment]::ExpandEnvironmentVariables($profile.ProfileImagePath)
                RemoveOwnedLink (Join-Path $homePath 'AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup')
            }
        }
        # Also honor explicit, redirected Startup folders for loaded user profiles.
        foreach($hive in Get-ChildItem 'Registry::HKEY_USERS'){
            $key=Join-Path $hive.PSPath 'Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders'
            $folders=Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
            if($folders.Startup -and [IO.Path]::IsPathRooted($folders.Startup) -and $folders.Startup -notmatch '%'){
                RemoveOwnedLink $folders.Startup
            }
        }
    }
} finally { [Runtime.InteropServices.Marshal]::ReleaseComObject($shell) | Out-Null }

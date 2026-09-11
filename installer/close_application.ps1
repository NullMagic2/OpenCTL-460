# Closes only OpenCTL executables in the selected installation directory before files are replaced.
# The explicit tray Exit message allows pen release; a bounded fallback terminates unresponsive copies.
param(
    [Parameter(Mandatory=$true)][string]$InstallDir,
    [ValidateRange(1,30)][int]$GraceSeconds=10
)
$ErrorActionPreference='Stop'
$directory=[IO.Path]::GetFullPath($InstallDir)
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class OpenCTLInstallerExit {
    private delegate bool EnumProc(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window, uint message, IntPtr wp, IntPtr lp);
    public static void Request(int pid) {
        // EnumWindows includes hidden tray windows; WM_CLOSE would merely hide current versions.
        EnumWindows((window,data) => {
            uint owner; GetWindowThreadProcessId(window,out owner);
            if (owner == pid) {
                var name=new StringBuilder(128); GetClassName(window,name,128);
                if (name.ToString() == "CTL460RustControlPanel")
                    PostMessage(window,0x801f,IntPtr.Zero,IntPtr.Zero);
            }
            return true;
        },IntPtr.Zero);
    }
}
'@
function Stop-InstalledProcess([string]$Name,[bool]$RequestExit) {
    $expected=Join-Path $directory ($Name+'.exe')
    foreach($process in [Diagnostics.Process]::GetProcessesByName($Name)) {
        try {
            if($process.HasExited){continue}
            # Keep this process handle open through validation and termination to avoid PID reuse.
            $null=$process.Handle
            $image=$process.MainModule.FileName
            if(-not [string]::Equals([IO.Path]::GetFullPath($image),$expected,[StringComparison]::OrdinalIgnoreCase)){continue}
            if($RequestExit){[OpenCTLInstallerExit]::Request($process.Id)}
            if(-not $process.WaitForExit($GraceSeconds*1000)) {
                Write-Output ('Terminating unresponsive '+$Name+' PID '+$process.Id)
                try {$process.Kill()} catch {if(-not $process.HasExited){throw}}
                if(-not $process.WaitForExit(5000)){throw "Cannot stop $Name before updating its files."}
            } else {Write-Output ('Closed '+$Name+' PID '+$process.Id)}
        } catch {
            if(-not $process.HasExited){throw}
        } finally {$process.Dispose()}
    }
}
Stop-InstalledProcess 'ctl460-gui' $true
# The GUI/supervisor normally stops the feeder; handle an orphaned or stalled feeder as well.
Stop-InstalledProcess 'ctl460-rust' $false

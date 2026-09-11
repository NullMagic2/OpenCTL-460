# Read-only Restart Manager query shared by the wizard and elevated setup helper.
# Never closes applications: users must save their work and close them themselves.
param([string[]]$Paths=@(
    (Join-Path $env:SystemRoot 'System32\Wintab32.dll'),
    (Join-Path $env:SystemRoot 'SysWOW64\Wintab32.dll')))

function Get-TabletApplications([string[]]$ResourcePaths) {
    $files=@($ResourcePaths | Where-Object {Test-Path -LiteralPath $_ -PathType Leaf} | Select-Object -Unique)
    if(-not $files.Count){return}
    if(-not ('OpenCTL.TabletAppCheck' -as [type])){
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
namespace OpenCTL {
    // Structures follow restartmanager.h; BOOL is four bytes and strings are UTF-16.
    public static class TabletAppCheck {
        [StructLayout(LayoutKind.Sequential)]
        public struct UniqueProcess {
            public uint Id;
            public System.Runtime.InteropServices.ComTypes.FILETIME StartTime;
        }
        [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
        public struct AppInfo {
            public UniqueProcess Process;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string Name;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst=64)] public string Service;
            public uint Type, Status, Session;
            [MarshalAs(UnmanagedType.Bool)] public bool Restartable;
        }
        [DllImport("rstrtmgr.dll", CharSet=CharSet.Unicode)]
        static extern int RmStartSession(out uint session, uint flags, StringBuilder key);
        [DllImport("rstrtmgr.dll", CharSet=CharSet.Unicode)]
        static extern int RmRegisterResources(uint session, uint count, string[] files,
            uint appCount, IntPtr apps, uint serviceCount, IntPtr services);
        [DllImport("rstrtmgr.dll")]
        static extern int RmGetList(uint session, out uint needed, ref uint count,
            [In, Out] AppInfo[] apps, out uint reasons);
        [DllImport("rstrtmgr.dll")]
        static extern int RmEndSession(uint session);
        static void Check(int error) {
            if(error != 0) throw new Win32Exception(error, "Cannot check applications using the tablet driver");
        }
        public static AppInfo[] Query(string[] files) {
            uint session;
            Check(RmStartSession(out session, 0, new StringBuilder(33)));
            try {
                Check(RmRegisterResources(session, (uint)files.Length, files, 0, IntPtr.Zero, 0, IntPtr.Zero));
                uint count=0, needed, reasons;
                AppInfo[] apps=null;
                // Processes can start or exit between the size query and the result query.
                for(int attempt=0; attempt<8; attempt++) {
                    int error=RmGetList(session, out needed, ref count, apps, out reasons);
                    if(error==0) {
                        if(count==0) return new AppInfo[0];
                        Array.Resize(ref apps, (int)count);
                        return apps;
                    }
                    if(error!=234) Check(error);
                    count=needed;
                    apps=new AppInfo[count];
                }
                throw new InvalidOperationException("The application list changed during the check. Please retry.");
            } finally { RmEndSession(session); }
        }
    }
}
'@ -ErrorAction Stop
    }
    [OpenCTL.TabletAppCheck]::Query([string[]]$files) | ForEach-Object {
        $label=$_.Name
        if([string]::IsNullOrWhiteSpace($label)){$label='Application'}
        '{0} (PID {1})' -f ($label -replace '[\r\n]+',' '),$_.Process.Id
    } | Sort-Object -Unique
}

function Assert-TabletApplicationsClosed([string[]]$ResourcePaths) {
    $apps=@(Get-TabletApplications $ResourcePaths)
    if($apps.Count){
        $failure=[Exception]::new("These applications are using the tablet driver:`r`n`r`n"+
            ($apps -join "`r`n")+"`r`n`r`nSave your work and fully close these applications, then retry setup.")
        $failure.Data['OpenCTLExitCode']=32
        throw $failure
    }
}

# Dot-sourcing exposes functions without running a check or exiting the caller.
if($MyInvocation.InvocationName -ne '.'){
    $ErrorActionPreference='Stop'
    try { Assert-TabletApplicationsClosed $Paths; exit 0 }
    catch {
        [Console]::Error.WriteLine($_.Exception.Message)
        if($_.Exception.Data['OpenCTLExitCode']){exit ([int]$_.Exception.Data['OpenCTLExitCode'])}
        exit 1
    }
}

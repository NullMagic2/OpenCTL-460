# Reads live pressure through our actual x64 WinTab DLL while another application uses Windows Ink.
# Creates a poll-only context; never injects input or installs/replaces a system DLL.
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Threading;
public class LiveWinTabProbe {
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern IntPtr LoadLibrary(string p);
 [DllImport("kernel32.dll")] static extern IntPtr GetProcAddress(IntPtr h,string n);
 [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate uint Info(uint c,uint i,IntPtr p);
 [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr Open(IntPtr w,IntPtr c,int e);
 [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int Packets(IntPtr c,int n,IntPtr p);
 [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int Close(IntPtr c);
 public static string Run(string path) {
  var h=LoadLibrary(path);if(h==IntPtr.Zero)throw new Exception("Cannot load provider");
  var info=(Info)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTInfoA"),typeof(Info));
  var open=(Open)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTOpenA"),typeof(Open));
  var get=(Packets)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTPacketsGet"),typeof(Packets));
  var close=(Close)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTClose"),typeof(Close));
  var context=Marshal.AllocHGlobal(172);var buffer=Marshal.AllocHGlobal(128*16);IntPtr c=IntPtr.Zero;
  int count=0,contacts=0,max=0;
  try {
   if(info(3,0,context)!=172)throw new Exception("Unexpected LOGCONTEXT ABI");
   Marshal.WriteInt32(context,40,0);Marshal.WriteInt32(context,40+6*4,0x5c0);Marshal.WriteInt32(context,40+8*4,0x5c0);
   c=open(IntPtr.Zero,context,1);if(c==IntPtr.Zero)throw new Exception("Cannot open context");
   var end=DateTime.UtcNow.AddSeconds(8);
   while(DateTime.UtcNow<end){int n=get(c,128,buffer);count+=n;for(int i=0;i<n;i++){int b=Marshal.ReadInt32(buffer,i*16),p=Marshal.ReadInt32(buffer,i*16+12);if((b&1)!=0)contacts++;max=Math.Max(max,p);}Thread.Sleep(2);}
  } finally {if(c!=IntPtr.Zero)close(c);Marshal.FreeHGlobal(context);Marshal.FreeHGlobal(buffer);}
  return "Live WinTab packets="+count+"; contact packets="+contacts+"; maximum virtual pressure="+max+"/4097";
 }
}
'@
$root=Split-Path -Parent $PSScriptRoot
$result=[LiveWinTabProbe]::Run((Join-Path $root 'dist\wintab\x64\Wintab32.dll'))
$result
$result | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'generated\live-wintab-result.txt') -Encoding UTF8

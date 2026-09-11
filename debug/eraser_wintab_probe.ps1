# Reads status/cursor/contact through the installed WinTab DLL to verify the held eraser signal.
# Creates a poll-only context; never injects input or installs/replaces a system DLL.
param([int]$Seconds=30)
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
 public static string Run(string path, int seconds) {
  var h=LoadLibrary(path);if(h==IntPtr.Zero)throw new Exception("Cannot load provider");
  var info=(Info)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTInfoA"),typeof(Info));
  var open=(Open)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTOpenA"),typeof(Open));
  var get=(Packets)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTPacketsGet"),typeof(Packets));
  var close=(Close)Marshal.GetDelegateForFunctionPointer(GetProcAddress(h,"WTClose"),typeof(Close));
  var context=Marshal.AllocHGlobal(172);var buffer=Marshal.AllocHGlobal(128*16);IntPtr c=IntPtr.Zero;
  int count=0,contacts=0,max=0,erasers=0,eraserContacts=0,penContacts=0;
  try {
   if(info(3,0,context)!=172)throw new Exception("Unexpected LOGCONTEXT ABI");
   Marshal.WriteInt32(context,40,0);Marshal.WriteInt32(context,40+6*4,0x462);Marshal.WriteInt32(context,40+8*4,0x462);
   c=open(IntPtr.Zero,context,1);if(c==IntPtr.Zero)throw new Exception("Cannot open context");
   var end=DateTime.UtcNow.AddSeconds(seconds);
   while(DateTime.UtcNow<end){int n=get(c,128,buffer);count+=n;for(int i=0;i<n;i++){int status=Marshal.ReadInt32(buffer,i*16),cursor=Marshal.ReadInt32(buffer,i*16+4),b=Marshal.ReadInt32(buffer,i*16+8),p=Marshal.ReadInt32(buffer,i*16+12); bool erase=(status&16)!=0 && cursor==1; if(erase)erasers++; if((b&1)!=0){contacts++;if(erase)eraserContacts++;else penContacts++;}max=Math.Max(max,p);}Thread.Sleep(2);}
  } finally {if(c!=IntPtr.Zero)close(c);Marshal.FreeHGlobal(context);Marshal.FreeHGlobal(buffer);}
  return "Live WinTab packets="+count+"; contact packets="+contacts+"; maximum virtual pressure="+max+"/4097; eraser packets="+erasers+"; eraser contacts="+eraserContacts+"; normal pen contacts="+penContacts;
 }
}
'@
$root=Split-Path -Parent $PSScriptRoot
$result=[LiveWinTabProbe]::Run((Join-Path $env:SystemRoot 'System32\Wintab32.dll'), $Seconds)
$result
$result | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'generated\eraser-wintab-result.txt') -Encoding UTF8

# Enumerates contexts without opening or consuming input. Optional explicit context raise supports routing diagnosis.
param([long]$RaiseContext=0)
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ContextInspector {
 [DllImport("Wintab32.dll", EntryPoint="WTMgrOpen")] static extern IntPtr Open(IntPtr w,uint b);
 [DllImport("Wintab32.dll", EntryPoint="WTMgrClose")] static extern int Close(IntPtr m);
 delegate int EnumProc(IntPtr c,IntPtr p);
 [DllImport("Wintab32.dll", EntryPoint="WTMgrContextEnum")] static extern int Enum(IntPtr m,EnumProc cb,IntPtr p);
 [DllImport("Wintab32.dll", EntryPoint="WTGetW")] static extern int Get(IntPtr c,IntPtr p);
 [DllImport("Wintab32.dll", EntryPoint="WTMgrContextOwner")] static extern IntPtr Owner(IntPtr m,IntPtr c);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("Wintab32.dll", EntryPoint="WTOverlap")] static extern int Overlap(IntPtr c,int top);
 public static void Run(long raise) {
  var m=Open(IntPtr.Zero,0);if(m==IntPtr.Zero)throw new Exception("manager unavailable");
  try {if(raise!=0 && Overlap(new IntPtr(raise),1)==0)throw new Exception("Cannot raise requested context");Enum(m, (c,a)=>{var b=Marshal.AllocHGlobal(212); try {if(Get(c,b)==0)return 1;uint pid;var w=Owner(m,c);GetWindowThreadProcessId(w,out pid);var words=new int[33];Marshal.Copy(IntPtr.Add(b,80),words,0,33);Console.WriteLine("context="+c+" pid="+pid+" window="+w+" name="+Marshal.PtrToStringUni(b,40).TrimEnd('\0')+" words="+string.Join(",",words));}finally{Marshal.FreeHGlobal(b);}return 1;},IntPtr.Zero);}finally{Close(m);}
 }
}
'@
[ContextInspector]::Run($RaiseContext)

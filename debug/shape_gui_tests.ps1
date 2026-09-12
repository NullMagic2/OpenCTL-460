# Exercises only our settings window in an isolated debug directory, without starting pen input.
# Checks pressure, handwriting, smoothing page visibility and slider persistence.
# Hidden-window captures can be blank and are not a visual acceptance test.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$generated = Join-Path $PSScriptRoot 'generated'
New-Item -ItemType Directory -Force -Path $generated | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class GuiTestNative {
 [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr w);
 [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr w,IntPtr dc);
 [DllImport("gdi32.dll")] static extern IntPtr SelectObject(IntPtr dc,IntPtr obj);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int DrawText(IntPtr dc,string text,int count,ref Rect rect,uint flags);
 public static bool LabelFits(IntPtr w) {
   Rect r;GetClientRect(w,out r);int available=r.r-r.l;
   var dc=GetDC(w);var font=SendMessage(w,0x31,IntPtr.Zero,IntPtr.Zero);var old=SelectObject(dc,font);
   try {var need=new Rect();DrawText(dc,Text(w),-1,ref need,0x420);return need.r-need.l<=available;}
   finally {SelectObject(dc,old);ReleaseDC(w,dc);}
 }
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string className,string title);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessage(string message);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process,uint flags);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowDpiAwarenessContext(IntPtr w);
 [DllImport("user32.dll")] public static extern bool AreDpiAwarenessContextsEqual(IntPtr a, IntPtr b);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr w);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr w,out Rect r);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr w);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr w);
 public static IntPtr FindPrompt(int pid) {IntPtr result=IntPtr.Zero; EnumWindows((w,p)=>{uint id;GetWindowThreadProcessId(w,out id);var b=new System.Text.StringBuilder(128);GetClassName(w,b,128);if(id==pid && b.ToString()=="OpenCTLPressureName") {result=w;return false;}return true;},IntPtr.Zero);return result;}
 public static IntPtr FindCapture(int pid) {IntPtr result=IntPtr.Zero; EnumWindows((w,p)=>{uint id;GetWindowThreadProcessId(w,out id);var b=new System.Text.StringBuilder(128);GetClassName(w,b,128);if(id==pid && b.ToString()=="OpenCTLShortcutCapture") {result=w;return false;}return true;},IntPtr.Zero);return result;}
 [StructLayout(LayoutKind.Sequential)] public struct Point {public int x,y;}
 [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr w,ref Point p);
 [DllImport("user32.dll")] public static extern IntPtr ChildWindowFromPointEx(IntPtr w,Point p,uint flags);
 public static bool ReceivesHit(IntPtr root,IntPtr child) {Rect r;GetWindowRect(child,out r);var p=new Point{x=(r.l+r.r)/2,y=(r.t+r.b)/2};ScreenToClient(root,ref p);return ChildWindowFromPointEx(root,p,3)==child;}
 public delegate bool EnumProc(IntPtr w, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr v);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr w, out uint p);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr w, System.Text.StringBuilder b, int n);
 public static IntPtr Find(int pid) {IntPtr result=IntPtr.Zero; EnumWindows((w,p)=>{uint id;GetWindowThreadProcessId(w,out id);var b=new System.Text.StringBuilder(128);GetClassName(w,b,128);if(id==pid && b.ToString()=="CTL460RustControlPanel") {result=w;return false;}return true;},IntPtr.Zero);return result;}
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr w, int id);
 [DllImport("user32.dll",EntryPoint="GetWindowLongW")] public static extern int GetWindowLong(IntPtr w, int index);
 // Check the child's own WS_VISIBLE bit: the root is deliberately hidden during testing.
 public static bool PageVisible(IntPtr w) { return (GetWindowLong(w,-16) & 0x10000000) != 0; }
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern bool SetWindowText(IntPtr w, string text);
 [DllImport("user32.dll",EntryPoint="SendMessageW",CharSet=CharSet.Unicode)] static extern IntPtr GetText(IntPtr w,uint m,IntPtr count,System.Text.StringBuilder text);
 public static string Text(IntPtr w) {var b=new System.Text.StringBuilder(512);GetText(w,0x0d,(IntPtr)b.Capacity,b);return b.ToString();}
 public static string ItemText(IntPtr w,int index) {var b=new System.Text.StringBuilder(512);GetText(w,0x148,(IntPtr)index,b);return b.ToString();}
 [DllImport("user32.dll",EntryPoint="SendMessageW",CharSet=CharSet.Unicode)] public static extern IntPtr SetText(IntPtr w, uint m, IntPtr a, string text);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr w, uint m, IntPtr a, IntPtr b);
 [StructLayout(LayoutKind.Sequential)] public struct Header { public IntPtr hwnd; public UIntPtr id; public uint code; }
 public static void Tab(IntPtr root, int page) {
   // TCM_SETCURFOCUS on ordinary tabs changes selection and sends a genuine in-process notification.
   var tab=GetDlgItem(root,170); SendMessage(tab,0x1330,(IntPtr)page,IntPtr.Zero);
 }
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr w, uint m, IntPtr a, IntPtr b);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr w, IntPtr dc, uint flags);
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int l,t,r,b; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr w, out Rect rect);
 [StructLayout(LayoutKind.Sequential)] public struct Monitor {public uint size; public Rect monitor,work;public uint flags;}
 [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr w,uint flags);
 [DllImport("user32.dll",EntryPoint="GetMonitorInfoW")] public static extern bool GetMonitorInfo(IntPtr monitor,ref Monitor info);
 public static bool Centered(IntPtr w) {Rect r;GetWindowRect(w,out r);var m=new Monitor{size=(uint)Marshal.SizeOf(typeof(Monitor))};GetMonitorInfo(MonitorFromWindow(w,1),ref m);return Math.Abs(r.l-(m.work.l+Math.Max(0,(m.work.r-m.work.l-(r.r-r.l))/2)))<=2 && Math.Abs(r.t-(m.work.t+Math.Max(0,(m.work.b-m.work.t-(r.b-r.t))/2)))<=2;}
}
'@
[GuiTestNative]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null

$generated=Join-Path $PSScriptRoot 'shape-generated'
New-Item -ItemType Directory -Force -Path $generated | Out-Null
[IO.File]::WriteAllText((Join-Path $generated 'settings.toml'),"handwriting_mode = 'off'")
$exe=Join-Path $projectRoot 'target/release/ctl460-gui.exe'
$appProcess=Start-Process -FilePath $exe -ArgumentList @('--settings-dir',('"'+$generated+'"')) -WindowStyle Hidden -PassThru
try {
 $deadline=[DateTime]::UtcNow.AddSeconds(10)
 do{Start-Sleep -Milliseconds 100;$window=[GuiTestNative]::Find($appProcess.Id)}while($window -eq 0 -and [DateTime]::UtcNow -lt $deadline)
 if($window -eq 0){throw 'Settings failed to open'}
 $tab=[GuiTestNative]::GetDlgItem($window,170)
 if([GuiTestNative]::SendMessage($tab,0x1304,[IntPtr]0,[IntPtr]0).ToInt32() -ne 6){throw 'Drawing aids tab missing'}
 [GuiTestNative]::Tab($window,5)
 foreach($id in @(710,711,712,713,714)){
   $w=[GuiTestNative]::GetDlgItem($window,$id)
   if($w -eq 0 -or -not [GuiTestNative]::PageVisible($w)){throw "Missing drawing aid $id"}
 }
 foreach($entry in @(@(710,1),@(711,0),@(712,1),@(714,1))){
  if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,$entry[0]),0xf0,[IntPtr]0,[IntPtr]0).ToInt32() -ne $entry[1]){throw 'Wrong aid default'}
 }
 foreach($id in @(710,712,714)){
  [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,$id),0xf1,[IntPtr]0,[IntPtr]0)|Out-Null
  [GuiTestNative]::SendMessage($window,0x111,[IntPtr]$id,[IntPtr]0)|Out-Null
 }
 [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,711),0xf1,[IntPtr]1,[IntPtr]0)|Out-Null
 [GuiTestNative]::SendMessage($window,0x111,[IntPtr]711,[IntPtr]0)|Out-Null
 [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($window,713),0x0c,[IntPtr]0,'0.9')|Out-Null
 Start-Sleep -Milliseconds 500
 $saved=Get-Content -Raw (Join-Path $generated 'settings.toml')
 foreach($text in @('endpoint_settling = false','close_shapes = true','show_start_marker = false','flowing_smoothing = false','closure_radius_mm = 0.9')){
   if(-not $saved.Contains($text)){throw "Drawing aid was not saved: $text"}
 }
 [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($window,713),0x0c,[IntPtr]0,'3')|Out-Null
 Start-Sleep -Milliseconds 400
 if((Get-Content -Raw (Join-Path $generated 'settings.toml')) -ne $saved){throw 'Invalid closure radius was saved'}
 [GuiTestNative]::Tab($window,3)
 if([GuiTestNative]::PageVisible([GuiTestNative]::GetDlgItem($window,711))){throw 'Drawing controls leaked onto smoothing page'}
 Write-Output 'PASS: Drawing aids tab, defaults, all switches, closure distance persistence, invalid input and page isolation.'
}finally{
 if(-not $appProcess.HasExited){[GuiTestNative]::PostMessage($window,0x801f,[IntPtr]0,[IntPtr]0)|Out-Null;if(-not $appProcess.WaitForExit(5000)){$appProcess.Kill()}}
}

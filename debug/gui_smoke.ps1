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
$exe = Join-Path $projectRoot 'target\release\ctl460-gui.exe'
$appProcess = Start-Process -FilePath $exe -ArgumentList @('--settings-dir', ('"' + $generated + '"')) -WindowStyle Hidden -PassThru
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do { Start-Sleep -Milliseconds 100; $appProcess.Refresh(); $window=[GuiTestNative]::Find($appProcess.Id) } while ($window -eq 0 -and -not $appProcess.HasExited -and [DateTime]::UtcNow -lt $deadline)
    if ($window -eq 0) { throw 'Settings window did not open' }
    if(-not [GuiTestNative]::Centered($window)){throw 'Settings window did not open centered in the monitor work area'}
    if(-not [GuiTestNative]::AreDpiAwarenessContextsEqual([GuiTestNative]::GetWindowDpiAwarenessContext($window),[IntPtr](-4))){throw 'GUI is not per-monitor DPI aware v2'}
    if([GuiTestNative]::Text([GuiTestNative]::GetDlgItem($window,224)) -ne 'Version 0.2.2'){throw 'Sidebar version missing or incorrect'}
    if([GuiTestNative]::GetDlgItem($window,140) -ne [IntPtr]0){throw 'Save settings button should be removed'}
    if([GuiTestNative]::GetDlgItem($window,213) -ne [IntPtr]0){throw 'Removed current-pressure control still exists'}
    if([GuiTestNative]::SendMessage($window,0x7f,[IntPtr]1,[IntPtr]0) -eq [IntPtr]0){throw 'Application icon is missing'}
    [GuiTestNative]::Tab($window,2)
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]216,[IntPtr]0) | Out-Null
    if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,125),0xf0,[IntPtr]0,[IntPtr]0) -ne [IntPtr]1){throw 'Left tablet picture did not select left-handed mapping'}
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]217,[IntPtr]0) | Out-Null
    if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,125),0xf0,[IntPtr]0,[IntPtr]0) -ne [IntPtr]0){throw 'Right tablet picture did not clear left-handed mapping'}
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,218),0x405,[IntPtr]1,[IntPtr]24) | Out-Null
    # Unsaved orientation/aspect choices survive selecting the simulated-pencil profile.
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,125),0xf1,[IntPtr]1,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,126),0xf1,[IntPtr]0,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,110),0x14e,[IntPtr]3,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](110 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,126),0xf0,[IntPtr]0,[IntPtr]0) -ne [IntPtr]0){throw 'Pencil profile changed the aspect mapping'}
    if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,125),0xf0,[IntPtr]0,[IntPtr]0) -ne [IntPtr]1){throw 'Pencil profile changed the orientation'}
    foreach ($entry in @(@(160,'10'),@(161,'850'),@(162,'1.25'),@(163,'5'),@(164,'2'),@(120,'0.9'))) {
        $control = [GuiTestNative]::GetDlgItem($window,[int]$entry[0])
        if ($control -eq 0) { throw "Missing pressure control $($entry[0])" }
        [GuiTestNative]::SetText($control,0x0c,[IntPtr]0,[string]$entry[1]) | Out-Null
    }
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,165),0x14e,[IntPtr]2,[IntPtr]0) | Out-Null
    # Artistic controls stay usable in manual and automatic handwriting as well as drawing.
    foreach ($mode in @(1,0,2)) {
        [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,165),0x14e,[IntPtr]$mode,[IntPtr]0) | Out-Null
        [GuiTestNative]::SendMessage($window,0x111,[IntPtr](165 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
        foreach ($id in 180..185) {
            if([GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($window,$id)) -ne $true){throw "Incorrect smoothing availability for mode $mode control $id"}
        }
    }
    # Select the native smoothing tab and deliver its selection notification.
    [GuiTestNative]::Tab($window,3)
    if ([GuiTestNative]::PageVisible([GuiTestNative]::GetDlgItem($window,160))) { throw 'Pressure page was not hidden' }
    foreach ($id in 180..185) {
        $slider=[GuiTestNative]::GetDlgItem($window,$id)
        if ($slider -eq 0 -or -not [GuiTestNative]::PageVisible($slider)) { throw "Missing visible slider $id" }
        [GuiTestNative]::SendMessage($slider,0x405,[IntPtr]1,[IntPtr]($id-143)) | Out-Null
    }
    # Button capture uses only local window messages, never global keyboard injection.
    # Portable tests create startup links inside debug/generated, never the real Startup folder.
    $startup=[GuiTestNative]::GetDlgItem($window,225)
    if($startup -eq 0){throw 'Startup checkbox missing'}
    [GuiTestNative]::SendMessage($startup,0xf1,[IntPtr]1,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]225,[IntPtr]0) | Out-Null
    $linkPath=Join-Path $generated 'startup\OpenCTL 460.lnk'
    if(-not(Test-Path -LiteralPath $linkPath)){throw 'Startup checkbox did not create a link'}
    $shell=New-Object -ComObject WScript.Shell
    $link=$shell.CreateShortcut($linkPath)
    if($link.TargetPath -ne $exe -or $link.Arguments -ne '--startup'){throw 'Startup link has wrong executable or arguments'}
    [Runtime.InteropServices.Marshal]::ReleaseComObject($shell) | Out-Null
    [GuiTestNative]::SendMessage($startup,0xf1,[IntPtr]0,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]225,[IntPtr]0) | Out-Null
    if(Test-Path -LiteralPath $linkPath){throw 'Disabling startup did not remove its link'}
    [GuiTestNative]::Tab($window,1)
    $button1=[GuiTestNative]::GetDlgItem($window,130)
    $button2=[GuiTestNative]::GetDlgItem($window,131)
    $shortcut1=[GuiTestNative]::GetDlgItem($window,240)
    [GuiTestNative]::SendMessage($button1,0x14e,[IntPtr]3,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](130 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    $actionCount=[GuiTestNative]::SendMessage($button1,0x146,[IntPtr]0,[IntPtr]0).ToInt32()
    if($actionCount -ne 11){throw "Expected only built-in actions, got $actionCount"}
    if(-not [GuiTestNative]::PageVisible($button1) -or -not [GuiTestNative]::ReceivesHit($window,$shortcut1)){throw 'Shortcut button hidden or covered'}
    $className=New-Object System.Text.StringBuilder(64)
    [GuiTestNative]::GetClassName($shortcut1,$className,64) | Out-Null
    if($className.ToString() -ne 'Button'){throw 'Shortcut control is still a textbox'}
    function Open-ShortcutCapture([int]$controlId) {
        [GuiTestNative]::PostMessage($window,0x111,[IntPtr]$controlId,[GuiTestNative]::GetDlgItem($window,$controlId)) | Out-Null
        $until=[DateTime]::UtcNow.AddSeconds(3)
        do {Start-Sleep -Milliseconds 30; $capture=[GuiTestNative]::FindCapture($appProcess.Id)} while($capture -eq 0 -and [DateTime]::UtcNow -lt $until)
        if($capture -eq 0){throw 'Shortcut capture did not open'}
        return $capture
    }
    [GuiTestNative]::SendMessage($button1,0x14e,[IntPtr]3,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](130 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    foreach($item in @(@(3,'Undo'),@(4,'Redo'),@(5,'Eraser tool'),@(6,'Brush'),@(7,'Pan (hold)'))) {
        if([GuiTestNative]::ItemText($button1,[int]$item[0]) -ne $item[1]){throw 'Action name still contains a keyboard shortcut'}
    }
    $capture=Open-ShortcutCapture 240
    if([GuiTestNative]::Text([GuiTestNative]::GetDlgItem($capture,10)) -notlike '*Ctrl+Z*'){throw 'Undo default missing in recorder'}
    # Render our own capture dialog for layout inspection.
    $captureBounds=New-Object GuiTestNative+Rect
    [GuiTestNative]::GetWindowRect($capture,[ref]$captureBounds) | Out-Null
    $captureImage=New-Object Drawing.Bitmap(($captureBounds.r-$captureBounds.l),($captureBounds.b-$captureBounds.t))
    $captureGraphics=[Drawing.Graphics]::FromImage($captureImage)
    $captureDc=$captureGraphics.GetHdc()
    try {[GuiTestNative]::PrintWindow($capture,$captureDc,2) | Out-Null} finally {$captureGraphics.ReleaseHdc($captureDc);$captureGraphics.Dispose()}
    $captureImage.Save((Join-Path $generated 'shortcut-dialog.png'),[Drawing.Imaging.ImageFormat]::Png)
    $captureImage.Dispose()
    foreach($key in @(0x11,0x10,0x5a)){[GuiTestNative]::PostMessage($capture,0x100,[IntPtr]$key,[IntPtr]1) | Out-Null}
    # Repeated keydown must not add another chord. Save waits for terminal-key release.
    [GuiTestNative]::PostMessage($capture,0x100,[IntPtr]0x5a,[IntPtr]0x40000001) | Out-Null
    Start-Sleep -Milliseconds 100
    if([GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($capture,1))){throw 'Recorder saved before key release'}
    foreach($key in @(0x5a,0x10,0x11)){[GuiTestNative]::PostMessage($capture,0x101,[IntPtr]$key,[IntPtr]1) | Out-Null}
    Start-Sleep -Milliseconds 100
    if([GuiTestNative]::Text([GuiTestNative]::GetDlgItem($capture,10)) -ne 'Ctrl+Shift+Z'){throw 'Incorrect captured chord'}
    [GuiTestNative]::SendMessage($capture,0x111,[IntPtr]1,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 300
    if([GuiTestNative]::SendMessage($button1,0x147,[IntPtr]0,[IntPtr]0).ToInt32() -ne -1 -or [GuiTestNative]::Text($button1) -ne 'Ctrl+Shift+Z'){throw 'Captured chord was not displayed independently of action choices'}
    if([GuiTestNative]::SendMessage($button1,0x146,[IntPtr]0,[IntPtr]0).ToInt32() -ne $actionCount){throw 'Recording added an unwanted dropdown option'}
    [GuiTestNative]::SendMessage($button2,0x14e,[IntPtr]2,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](131 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 300
    $buttonSettings=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if(-not $buttonSettings.Contains('button1 = "Shortcut: Ctrl+Shift+Z"') -or -not $buttonSettings.Contains('button2 = "Erase (hold)"')){throw 'Button assignments did not persist'}
    # Held actions, mouse clicks and None must not open the keyboard recorder.
    foreach($actionIndex in @(0,1,2,7,8,9,10)) {
        [GuiTestNative]::SendMessage($button2,0x14e,[IntPtr]$actionIndex,[IntPtr]0) | Out-Null
        [GuiTestNative]::SendMessage($window,0x111,[IntPtr](131 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
        if([GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($window,241))){throw "Shortcut enabled for non-keyboard action $actionIndex"}
        [GuiTestNative]::SendMessage($window,0x111,[IntPtr]241,[IntPtr]0) | Out-Null
        if([GuiTestNative]::FindCapture($appProcess.Id) -ne 0){throw 'Disabled shortcut still opened recorder'}
    }
    [GuiTestNative]::SendMessage($button2,0x14e,[IntPtr]4,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](131 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    if(-not [GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($window,241))){throw 'Redo shortcut is not editable'}
    Start-Sleep -Milliseconds 300
    $buttonSettings=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    $capture=Open-ShortcutCapture 241
    [GuiTestNative]::SendMessage($capture,0x111,[IntPtr]3,[IntPtr]0) | Out-Null
    # A modifier alone is not a shortcut. Cancel must keep the previous assignment.
    [GuiTestNative]::SendMessage($capture,0x100,[IntPtr]0x11,[IntPtr]1) | Out-Null
    if([GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($capture,1))){throw 'Modifier-only shortcut accepted'}
    [GuiTestNative]::SendMessage($capture,0x111,[IntPtr]2,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 300
    if((Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')) -ne $buttonSettings){throw 'Cancel changed assignments'}
    $capture=Open-ShortcutCapture 241
    # Losing focus while Ctrl is held must not leave it latched in the next capture.
    [GuiTestNative]::SendMessage($capture,0x100,[IntPtr]0x11,[IntPtr]1) | Out-Null
    [GuiTestNative]::SendMessage($capture,6,[IntPtr]0,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($capture,0x100,[IntPtr]0x41,[IntPtr]1) | Out-Null
    [GuiTestNative]::SendMessage($capture,0x101,[IntPtr]0x41,[IntPtr]1) | Out-Null
    if([GuiTestNative]::Text([GuiTestNative]::GetDlgItem($capture,10)) -ne 'A'){throw 'Focus loss left a modifier latched'}
    [GuiTestNative]::SendMessage($capture,0x111,[IntPtr]3,[IntPtr]0) | Out-Null
    foreach($key in @(0x12,0x25)){[GuiTestNative]::PostMessage($capture,0x104,[IntPtr]$key,[IntPtr]1) | Out-Null}
    foreach($key in @(0x25,0x12)){[GuiTestNative]::PostMessage($capture,0x105,[IntPtr]$key,[IntPtr]1) | Out-Null}
    Start-Sleep -Milliseconds 80
    if([GuiTestNative]::Text([GuiTestNative]::GetDlgItem($capture,10)) -ne 'Alt+Left'){throw 'Alt combination was not captured'}
    foreach($key in @(13,27,32,9)) {
        # Enter, Escape, Space and Tab are captured as keys rather than dialog commands.
        [GuiTestNative]::SendMessage($capture,0x111,[IntPtr]3,[IntPtr]0) | Out-Null
        [GuiTestNative]::PostMessage($capture,0x100,[IntPtr]$key,[IntPtr]1) | Out-Null
        [GuiTestNative]::PostMessage($capture,0x101,[IntPtr]$key,[IntPtr]1) | Out-Null
        Start-Sleep -Milliseconds 80
        if([GuiTestNative]::FindCapture($appProcess.Id) -ne $capture -or -not [GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($capture,1))){throw 'Single key closed the recorder or failed to capture'}
    }
    [GuiTestNative]::SendMessage($capture,0x111,[IntPtr]1,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 300
    if((Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')) -notlike '*button2 = "Shortcut: Tab"*'){throw 'Single-key shortcut did not persist'}
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]244,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 300
    $resetButtons=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if(-not $resetButtons.Contains('button1 = "Right click"') -or -not $resetButtons.Contains('button2 = "None"')){throw 'Reset settings did not restore button defaults'}
    $withoutButtons='(?m)^button[12] = .*\r?\n'
    if(($resetButtons -replace $withoutButtons,'') -ne ($buttonSettings -replace $withoutButtons,'')){throw 'Button reset changed unrelated settings'}
    [GuiTestNative]::Tab($window,3)
    if([GuiTestNative]::PageVisible($button1)){throw 'Button controls leaked onto smoothing page'}
    # Let the real periodic autosave apply the edit without any command or button.
    Start-Sleep -Milliseconds 500
    $settings = Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if($settings -notmatch 'double_click_distance = 24'){throw 'Double-click distance was not preserved/saved'}
    foreach ($expected in @('floor = 10','ceiling = 850','gain = 1.25','press_on = 5','press_off = 2','handwriting_mode = "auto"','streamline_amount = 37.0','streamline_pressure = 38.0','stabilization_amount = 39.0','motion_filter_amount = 40.0','motion_filter_expression = 41.0','wobble_reduction = 42.0')) {
        if (-not $settings.Contains($expected)) {throw "Setting did not persist: $expected"}
    }
    # A partially typed number must keep the last valid persisted settings intact.
    [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($window,120),0x0c,[IntPtr]0,'-') | Out-Null
    Start-Sleep -Milliseconds 400
    if((Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')) -ne $settings){throw 'Invalid edit replaced valid settings'}
    [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($window,120),0x0c,[IntPtr]0,'0.9') | Out-Null
    Start-Sleep -Milliseconds 200
    [GuiTestNative]::Tab($window,0)
    $curve=[GuiTestNative]::GetDlgItem($window,210)
    if ($curve -eq 0 -or -not [GuiTestNative]::PageVisible($curve)) {throw 'Pen tab pressure editor missing'}
    # Drag our own graph; this is window-local mouse messaging, not global input injection.
    $dpi=[GuiTestNative]::GetDpiForWindow($window)/96.0
    $bounds=New-Object GuiTestNative+Rect
    [GuiTestNative]::GetClientRect($curve,[ref]$bounds) | Out-Null
    $x=[int](48*$dpi+(400.0/1023)*($bounds.r-68*$dpi))
    $output=[Math]::Pow((400.0-10)/840,0.75)*1.25
    $y=[int]($bounds.b-42*$dpi-$output*($bounds.b-76*$dpi))
    $position=[IntPtr]($x -bor ($y -shl 16))
    [GuiTestNative]::SendMessage($curve,0x201,[IntPtr]1,$position) | Out-Null
    [GuiTestNative]::SendMessage($curve,0x202,[IntPtr]0,$position) | Out-Null
    Start-Sleep -Milliseconds 150
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $settings = Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    $gamma=[double]::Parse([regex]::Match($settings,'(?m)^gamma = ([0-9.]+)').Groups[1].Value,[Globalization.CultureInfo]::InvariantCulture)
    if ($gamma -le 0.6 -or $gamma -ge 0.9) {throw "Pressure drag did not save expected response: $gamma"}
    # Exercise buffered redraws repeatedly; temporary GDI bitmaps/DCs must be freed.
    # Pixel inspection uses the app's in-process --preview path, since cross-process
    # hidden-window captures can be blank on some desktops.
    $surface=New-Object System.Drawing.Bitmap($bounds.r,$bounds.b)
    $render=[Drawing.Graphics]::FromImage($surface)
    $renderDc=$render.GetHdc()
    try {
        # Warm up Windows theme/font caches before measuring steady-state redraws.
        for($warmup=0;$warmup -lt 16;$warmup++) {
            [GuiTestNative]::SendMessage($curve,0x100,[IntPtr](0x25+2*($warmup%2)),[IntPtr]0) | Out-Null
            [GuiTestNative]::PrintWindow($curve,$renderDc,3) | Out-Null
        }
        Start-Sleep -Milliseconds 150
        $before=[GuiTestNative]::GetGuiResources($appProcess.Handle,0)
        for($frame=0;$frame -lt 64;$frame++) {
            [GuiTestNative]::SendMessage($curve,0x100,[IntPtr](0x25+2*($frame%2)),[IntPtr]0) | Out-Null
            [GuiTestNative]::PrintWindow($curve,$renderDc,3) | Out-Null
        }
        $after=[GuiTestNative]::GetGuiResources($appProcess.Handle,0)
        if($after -gt $before+2){throw "Curve repaint leaked GDI resources: $before -> $after"}
    } finally {$render.ReleaseHdc($renderDc);$render.Dispose()}
    $surface.Dispose()
    # Default cannot be edited/deleted. New creates editable nodes, save prompts, delete removes only custom data.
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,230),0x14e,[IntPtr]0,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](230 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    if(-not [GuiTestNative]::IsWindowEnabled($curve) -or [GuiTestNative]::IsWindowEnabled([GuiTestNative]::GetDlgItem($window,234))){throw 'Default must allow editing a copy but must not allow deletion'}
    if(-not [GuiTestNative]::ReceivesHit($window,$curve)){throw 'Default curve is covered by a decoration'}
    [GuiTestNative]::SendMessage($curve,0x201,[IntPtr]1,$position) | Out-Null
    [GuiTestNative]::SendMessage($curve,0x202,[IntPtr]0,$position) | Out-Null
    Start-Sleep -Milliseconds 150
    if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,230),0x147,[IntPtr]0,[IntPtr]0) -ne [IntPtr]1){throw 'Editing Default did not create an unsaved copy'}
    # The first Add on a legacy gamma curve must convert its three visible handles
    # and insert just one node. Subsequent clicks must increase the count by one.
    foreach($expectedCount in @(4,5)) {
        [GuiTestNative]::SendMessage($window,0x111,[IntPtr]236,[IntPtr]0) | Out-Null
        Start-Sleep -Milliseconds 100
        [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
        $savedCurve=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
        $nodeCount=[regex]::Matches($savedCurve,'\[\[pressure_nodes\]\]').Count
        if($nodeCount -ne $expectedCount){throw "Add node should produce $expectedCount nodes, got $nodeCount"}
    }
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,230),0x14e,[IntPtr]0,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](230 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $restored=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if($restored -notmatch '(?m)^gamma = 1(?:\.0)?$' -or $restored -notmatch '(?m)^ceiling = 1023$'){throw 'Editing the copy modified Default'}
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]232,[IntPtr]0) | Out-Null
    if(-not [GuiTestNative]::IsWindowEnabled($curve)){throw 'New curve is not editable'}
    if(-not [GuiTestNative]::ReceivesHit($window,$curve)){throw 'Another control intercepts clicks on the curve'}
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $newCurve=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if([regex]::Matches($newCurve,'\[\[pressure_nodes\]\]').Count -ne 3){throw 'New should create only the three starting handles'}
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]236,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 100
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]237,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 100
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $removedCurve=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if([regex]::Matches($removedCurve,'\[\[pressure_nodes\]\]').Count -ne 3){throw 'Add then Remove should restore the three starting handles'}
    # Reset clears custom shaping into an unsaved linear curve, including gain/range.
    $reset=[GuiTestNative]::GetDlgItem($window,212)
    if($reset -eq 0 -or -not [GuiTestNative]::PageVisible($reset)){throw 'Reset curve button is missing'}
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]212,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $linear=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    foreach($pattern in @('(?m)^gamma = 1(?:\.0)?$','(?m)^gain = 1(?:\.0)?$','(?m)^floor = 0$','(?m)^ceiling = 1023$')) {
        if($linear -notmatch $pattern){throw "Reset curve did not restore linear response: $pattern"}
    }
    if([GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,230),0x147,[IntPtr]0,[IntPtr]0) -ne [IntPtr]1){throw 'Reset must edit an unsaved copy'}
    # Grab the OUTER half of each endpoint; moving it must edit calibration, not add a node.
    $left=[int][Math]::Round(48*$dpi);$right=$bounds.r-[int][Math]::Round(20*$dpi)
    $top=[int][Math]::Round(34*$dpi);$bottom=$bounds.b-[int][Math]::Round(42*$dpi)
    foreach($endpoint in @(@($right,($top-[int](4*$dpi)),[int]($left+0.75*($right-$left)),$top),@(($left-[int](4*$dpi)),$bottom,[int]($left+0.2*($right-$left)),$bottom))) {
        $down=[IntPtr]($endpoint[0] -bor ($endpoint[1] -shl 16))
        $move=[IntPtr]($endpoint[2] -bor ($endpoint[3] -shl 16))
        [GuiTestNative]::SendMessage($curve,0x201,[IntPtr]1,$down) | Out-Null
        [GuiTestNative]::SendMessage($curve,0x200,[IntPtr]1,$move) | Out-Null
        [GuiTestNative]::SendMessage($curve,0x202,[IntPtr]0,$move) | Out-Null
        Start-Sleep -Milliseconds 150
    }
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $endpoints=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    $low=[int][regex]::Match($endpoints,'(?m)^floor = (\d+)').Groups[1].Value
    $high=[int][regex]::Match($endpoints,'(?m)^ceiling = (\d+)').Groups[1].Value
    if($low -lt 200 -or $low -gt 210 -or $high -lt 763 -or $high -gt 772){throw "Endpoint drag failed: $low..$high"}
    if([regex]::Matches($endpoints,'\[\[pressure_nodes\]\]').Count -ne 3){throw 'Endpoint drag inserted a node'}
    # New must start linear even after editing a shortened calibration range.
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]232,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $newLinear=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if($newLinear -notmatch '(?m)^floor = 0$' -or $newLinear -notmatch '(?m)^ceiling = 1023$' -or $newLinear -notmatch '(?m)^x = 0.5$' -or $newLinear -notmatch '(?m)^y = 0.5$'){throw 'New did not restore a full-range diagonal with a centered handle'}
    [GuiTestNative]::PostMessage($window,0x111,[IntPtr]233,[IntPtr]0) | Out-Null
    $deadline=[DateTime]::UtcNow.AddSeconds(5)
    do {Start-Sleep -Milliseconds 100;$dialog=[GuiTestNative]::FindPrompt($appProcess.Id)} while($dialog -eq 0 -and [DateTime]::UtcNow -lt $deadline)
    if($dialog -eq 0){throw 'Pressure preset name dialog missing'}
    $presetName='GUI test '+[Guid]::NewGuid().ToString('N').Substring(0,8)
    [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($dialog,10),0x0c,[IntPtr]0,$presetName) | Out-Null
    [GuiTestNative]::PostMessage($dialog,0x111,[IntPtr]1,[IntPtr]0) | Out-Null
    Start-Sleep -Milliseconds 500
    $library=Join-Path $generated 'pressure-profiles.toml'
    if(-not (Get-Content -Raw $library).Contains($presetName)){throw 'Named preset was not saved'}
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]234,[IntPtr]0) | Out-Null
    if((Get-Content -Raw $library).Contains($presetName)){throw 'Custom preset was not deleted'}
    # Reset is also available on protected Default without changing that preset.
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr]212,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage([GuiTestNative]::GetDlgItem($window,230),0x14e,[IntPtr]0,[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x111,[IntPtr](230 -bor (1 -shl 16)),[IntPtr]0) | Out-Null
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]1,[IntPtr]0) | Out-Null
    $protected=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    if($protected -notmatch '(?m)^gamma = 1(?:\.0)?$' -or $protected -notmatch '(?m)^ceiling = 1023$'){throw 'Reset modified the built-in Default'}
    $rect = New-Object GuiTestNative+Rect
    [GuiTestNative]::GetWindowRect($window,[ref]$rect) | Out-Null
    $bitmap = New-Object System.Drawing.Bitmap(($rect.r-$rect.l),($rect.b-$rect.t))
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $dc = $graphics.GetHdc()
    try {
        # WM_PRINT renders this hidden test window and its children without capturing the desktop.
        [GuiTestNative]::PrintWindow($window,$dc,2) | Out-Null
    }
    finally {$graphics.ReleaseHdc($dc)}
    $bitmap.Save((Join-Path $generated 'settings.png'))
    $graphics.Dispose(); $bitmap.Dispose()
    # Closing hides settings without terminating the process; the tray reopens the
    # same window. Simulate Explorer's recovery notification without restarting it.
    [GuiTestNative]::SendMessage($window,0x8020,[IntPtr]0,[IntPtr]0) | Out-Null
    if(-not [GuiTestNative]::PageVisible($window)){throw 'Open settings failed'}
    # An incomplete edit must not block Close or replace the last valid saved settings.
    $beforeClose=Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')
    $previousGain=[GuiTestNative]::Text([GuiTestNative]::GetDlgItem($window,120))
    [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($window,120),0x0c,[IntPtr]0,'-') | Out-Null
    # WM_CLOSE hides after registration, or minimizes while Explorer is unavailable.
    $hasShell=[GuiTestNative]::FindWindow('Shell_TrayWnd',$null) -ne [IntPtr]0
    [GuiTestNative]::SendMessage($window,0x10,[IntPtr]0,[IntPtr]0) | Out-Null
    if($appProcess.HasExited){throw 'Close unexpectedly terminated settings'}
    if($hasShell) {
        if([GuiTestNative]::PageVisible($window)){throw 'Close did not hide settings in the tray'}
        [GuiTestNative]::SendMessage($window,[GuiTestNative]::RegisterWindowMessage('TaskbarCreated'),[IntPtr]0,[IntPtr]0) | Out-Null
        if([GuiTestNative]::PageVisible($window)){throw 'Tray icon recovery failed and forced settings open'}
    } else {
        # Sandboxed test desktops have no Explorer. Never hide the only reachable UI.
        if(-not [GuiTestNative]::PageVisible($window) -or -not [GuiTestNative]::IsIconic($window)){throw 'No-shell fallback must minimize settings to the taskbar'}
        [GuiTestNative]::SendMessage($window,0x113,[IntPtr]2,[IntPtr]0) | Out-Null
        if(-not [GuiTestNative]::IsIconic($window)){throw 'Tray retry restored the window unexpectedly'}
        Write-Output 'SKIP: real tray registration/hiding requires an Explorer desktop; no-shell fallback verified.'
    }
    [GuiTestNative]::SendMessage($window,0x801e,[IntPtr]0,[IntPtr](0x10000 -bor 0x400)) | Out-Null
    Start-Sleep -Milliseconds 100
    if(-not [GuiTestNative]::PageVisible($window) -or [GuiTestNative]::IsIconic($window)){throw 'Tray activation did not restore settings'}
    if((Get-Content -Raw -LiteralPath (Join-Path $generated 'settings.toml')) -ne $beforeClose){throw 'Invalid close changed valid settings'}
    if([GuiTestNative]::Text([GuiTestNative]::GetDlgItem($window,120)) -ne '-'){throw 'Close discarded the pending edit'}
    [GuiTestNative]::SendMessage($window,0x113,[IntPtr]2,[IntPtr]0) | Out-Null
    if(-not [GuiTestNative]::PageVisible($window) -or [GuiTestNative]::IsIconic($window)){throw 'A tray retry hid a restored window'}
    [GuiTestNative]::SetText([GuiTestNative]::GetDlgItem($window,120),0x0c,[IntPtr]0,$previousGain) | Out-Null
    [GuiTestNative]::PostMessage($window,0x801f,[IntPtr]0,[IntPtr]0) | Out-Null
    if(-not $appProcess.WaitForExit(5000)){throw 'Explicit tray Exit did not terminate settings'}
    Write-Output 'PASS: single-node insertion, tray callbacks/exit, buffered redraws without GDI leaks, Default protection, DPI, presets and persistence.'
} finally {
    if (-not $appProcess.HasExited) {
        [GuiTestNative]::PostMessage([GuiTestNative]::Find($appProcess.Id),0x801f,[IntPtr]0,[IntPtr]0) | Out-Null
        if (-not $appProcess.WaitForExit(5000)) { $appProcess.Kill() }
    }
}

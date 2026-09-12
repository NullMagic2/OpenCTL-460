; Builds an administrator setup wizard with mandatory 32/64-bit WinTab and per-user GUI settings.
; Existing WinTab DLLs are backed up by hash and restored only while our installed copy is unchanged.
#define AppVersion "0.3.21"
[Setup]
AppId={{A5BA4A5A-2694-4982-9874-3BAC1CE7B92D}
AppName=OpenCTL 460
AppVersion={#AppVersion}
AppPublisher=CTL-460 Rust contributors
DefaultDirName={autopf}\CTL460Studio
; Always show the folder picker, including when setup detects an earlier installation.
DisableDirPage=no
DefaultGroupName=OpenCTL 460
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
UsePreviousPrivileges=no
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.22000
OutputDir=..\dist
OutputBaseFilename=OpenCTL460-Setup-{#AppVersion}-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\assets\app.ico
LicenseFile=..\LICENSE.txt
InfoBeforeFile=INSTALL-NOTES.txt
UninstallDisplayIcon={app}\ctl460-gui.exe
CloseApplications=yes
RestartApplications=no
SetupLogging=yes

[Messages]
PrivilegesRequiredOverrideText1=Choose who can use %1. Windows tablet components require administrator approval in either mode.
PrivilegesRequiredOverrideText2=Choose who can use %1. Windows tablet components require administrator approval in either mode.
PrivilegesRequiredOverrideAllUsers=All users
PrivilegesRequiredOverrideAllUsersRecommended=All users (recommended)
PrivilegesRequiredOverrideCurrentUser=Just me
PrivilegesRequiredOverrideCurrentUserRecommended=Just me (recommended)

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
; Extract this installer-owned helper before touching existing application files.
Source: "close_application.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion
; The same read-only check is extracted before installation and kept for repairs.
Source: "..\scripts\check_tablet_apps.ps1"; DestDir: "{app}\scripts"; Flags: ignoreversion
Source: "..\dist\ctl460-rust.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\ctl460-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\ctl460-setup.exe"; DestDir: "{app}"; Flags: ignoreversion
; Keep the LocalSystem executable under the protected Common Files tree even with a custom app path.
Source: "..\dist\ctl460-service.exe"; DestDir: "{app}\system-package"; Flags: ignoreversion
Source: "..\dist\scripts\*.ps1"; DestDir: "{app}\scripts"; Excludes: "check_tablet_apps.ps1"; Flags: ignoreversion
Source: "..\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\dist\wintab\x64\Wintab32.dll"; DestDir: "{app}\system-package\x64"; Flags: ignoreversion
Source: "..\dist\wintab\x86\Wintab32.dll"; DestDir: "{app}\system-package\x86"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\profiles\*.toml"; DestDir: "{app}\profiles"; Flags: ignoreversion
Source: "..\docs\*.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "..\third-party-licenses\*.txt"; DestDir: "{app}\third-party-licenses"; Flags: ignoreversion
#if FileExists("..\dist\kernel-test\ctl460_vhf.sys")
Source: "..\dist\kernel-test\*"; DestDir: "{app}\kernel-test"; Flags: ignoreversion
#endif

[Icons]
Name: "{autoprograms}\OpenCTL 460"; Filename: "{app}\ctl460-gui.exe"
Name: "{group}\Uninstall OpenCTL 460"; Filename: "{uninstallexe}"
Name: "{autodesktop}\OpenCTL 460"; Filename: "{app}\ctl460-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ctl460-gui.exe"; Description: "Open OpenCTL 460"; Flags: nowait postinstall skipifsilent runasoriginaluser

[Code]
var PreviousAppDir: String;
    PreviousScopeKnown, PreviousAllUsers, KernelRestart: Boolean;
    SetupDetails: String;

procedure CaptureSetupOutput(const S: String; const Error, FirstLine: Boolean);
begin
  Log(S);
  if Length(SetupDetails) < 2000 then SetupDetails := SetupDetails + S + #13#10;
end;

function InitializeSetup: Boolean;
var Key: String;
begin
  Key := 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{A5BA4A5A-2694-4982-9874-3BAC1CE7B92D}_is1';
  PreviousScopeKnown := RegQueryStringValue(HKLM64, Key, 'Inno Setup: App Path', PreviousAppDir);
  PreviousAllUsers := PreviousScopeKnown;
  if not PreviousScopeKnown then
    PreviousScopeKnown := RegQueryStringValue(HKCU, Key, 'Inno Setup: App Path', PreviousAppDir);
  if PreviousAppDir = '' then
    RegQueryStringValue(HKLM64, 'Software\OpenCTL460', 'InstallPath', PreviousAppDir);
  Result := True;
end;

procedure RegisterExtraCloseApplicationsResources;
begin
  { These are installed by the elevated helper rather than [Files], so register
    their real destinations explicitly with Restart Manager (Inno 6.x API). }
  RegisterExtraCloseApplicationsResource(True, ExpandConstant('{sys}\Wintab32.dll'));
  RegisterExtraCloseApplicationsResource(True, ExpandConstant('{syswow64}\Wintab32.dll'));
end;

procedure InitializeWizard;
var Note: TNewStaticText;
begin
  Note := TNewStaticText.Create(WizardForm);
  Note.Parent := WizardForm.SelectDirPage;
  Note.Left := 0;
  Note.Top := WizardForm.DirEdit.Top + WizardForm.DirEdit.Height + ScaleY(32);
  Note.Width := WizardForm.SelectDirPage.ClientWidth;
  Note.Height := ScaleY(64);
  Note.AutoSize := False;
  Note.WordWrap := True;
  Note.Caption := 'Windows tablet components are shared and require administrator approval in either installation mode.';
end;

function InstallationPathError: String;
begin
  Result := '';
  if PreviousAppDir <> '' then
    if (CompareText(AddBackslash(ExpandFileName(PreviousAppDir)),
        AddBackslash(ExpandFileName(WizardDirValue))) <> 0) or
       (PreviousScopeKnown and (PreviousAllUsers <> IsAdminInstallMode)) then
      Result := 'To change the installation scope or folder, uninstall the existing OpenCTL 460 copy first, then run setup again. Current folder: ' + PreviousAppDir;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var Error: String;
begin
  Result := True;
  if CurPageID = wpSelectDir then begin
    Error := InstallationPathError;
    if Error <> '' then begin MsgBox(Error, mbError, MB_OK); Result := False; end;
  end;
end;

function CloseApplication(Script: String): Boolean;
var Code: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + Script +
    '" -InstallDir "' + AddBackslash(ExpandConstant('{app}')) + '."',
    '', SW_HIDE, ewWaitUntilTerminated, Code);
  if Result then Result := Code = 0;
end;

function TabletApplicationsError(Script: String): String;
var Code: Integer;
    Started: Boolean;
begin
  { Read-only in both installation scopes; no elevation or application shutdown. }
  SetupDetails := '';
  Started := ExecAndLogOutput(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + Script + '"',
    '', SW_HIDE, ewWaitUntilTerminated, Code, @CaptureSetupOutput);
  Result := '';
  if not Started then
    Result := 'Cannot check running tablet applications. Please retry setup.'
  else if Code <> 0 then begin
    Result := SetupDetails;
    if Result = '' then Result := 'Cannot check running tablet applications. Close drawing applications and retry setup.';
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := InstallationPathError;
  if Result <> '' then exit;
  try
    { Gate before application files, registry entries, or services are changed.
      Retry performs a fresh query. Silent setup fails without closing user apps. }
    ExtractTemporaryFile('check_tablet_apps.ps1');
    Result := TabletApplicationsError(ExpandConstant('{tmp}\check_tablet_apps.ps1'));
    while Result <> '' do begin
      if SuppressibleMsgBox(Result + #13#10 + #13#10 + 'Click Retry when ready, or Cancel to stop this installation attempt.',
        mbInformation, MB_RETRYCANCEL, IDCANCEL) <> IDRETRY then exit;
      Result := TabletApplicationsError(ExpandConstant('{tmp}\check_tablet_apps.ps1'));
    end;
    ExtractTemporaryFile('close_application.ps1');
    if not CloseApplication(ExpandConstant('{tmp}\close_application.ps1')) then
      RaiseException('Cannot stop the previous OpenCTL application. Close it and retry setup.');
  except Result := GetExceptionMessage; end;
end;

function SystemSetup(Action: String): Integer;
var Parameters: String;
    Started: Boolean;
begin
  SetupDetails := '';
  Parameters := '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' +
    ExpandConstant('{app}\scripts\system_setup.ps1') + '" -Action ' + Action +
    ' -AppDir "' + AddBackslash(ExpandConstant('{app}')) + '."';
  if IsAdminInstallMode then
    Started := ExecAndLogOutput(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      Parameters, '', SW_HIDE, ewWaitUntilTerminated, Result, @CaptureSetupOutput)
  else
    Started := ShellExec('runas', ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      Parameters, '', SW_HIDE, ewWaitUntilTerminated, Result);
  if not Started then Result := -1;
end;

procedure CurStepChanged(Step: TSetupStep);
var Code: Integer;
    AppError: String;
begin
  if Step = ssPostInstall then begin
    Code := SystemSetup('Install');
    while Code = 32 do begin
      { Covers an app opened after the preflight, including Just me installs
        where the elevated helper's output cannot be captured by the wizard. }
      AppError := TabletApplicationsError(ExpandConstant('{app}\scripts\check_tablet_apps.ps1'));
      if AppError = '' then AppError := 'WinTab is still in use. Save your work and close drawing applications, then click Retry.';
      if SuppressibleMsgBox(AppError, mbInformation, MB_RETRYCANCEL, IDCANCEL) <> IDRETRY then
        RaiseException('Installation was not completed because WinTab is in use. Close drawing applications and run setup again.');
      Code := SystemSetup('Install');
    end;
    if (Code <> 0) and (Code <> 3010) then
      RaiseException('Cannot install Windows tablet components. If setup was cancelled, run setup again and approve the Windows prompt. Otherwise close drawing applications and check ' + ExpandConstant('{app}\system-setup.log') + #13#10 + SetupDetails);
    KernelRestart := Code = 3010;
  end;
end;

function NeedRestart: Boolean;
begin Result := KernelRestart; end;

procedure CurUninstallStepChanged(Step: TUninstallStep);
var Code: Integer;
begin
  if Step <> usUninstall then exit;
  if not CloseApplication(ExpandConstant('{app}\scripts\close_application.ps1')) then begin
    MsgBox('Cannot stop OpenCTL 460. Close it and retry uninstall.', mbError, MB_OK); Abort;
  end;
  Code := SystemSetup('Remove');
  if (Code <> 0) and (Code <> 3010) then begin
    MsgBox('Windows tablet components could not be removed. The application has been kept so you can retry uninstall. See ' + ExpandConstant('{app}\system-setup.log'), mbError, MB_OK); Abort;
  end;
end;


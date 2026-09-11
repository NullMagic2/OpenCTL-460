@echo off
rem Builds the app, then compiles the Inno Setup wizard. Set ISCC to a custom compiler path.
setlocal
pushd "%~dp0"
call build_driver.bat
if errorlevel 1 goto failed
if defined ISCC goto compile
if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if defined ISCC goto compile
if exist "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" set "ISCC=%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe"
if defined ISCC goto compile
for %%I in (ISCC.exe) do set "ISCC=%%~$PATH:I"
if not defined ISCC goto missing
:compile
"%ISCC%" "installer\ctl460.iss"
if errorlevel 1 goto failed
echo Installer created in dist.
popd
exit /b 0
:missing
echo Install Inno Setup 6.7 or set ISCC to the full path of ISCC.exe.
:failed
popd
exit /b 1

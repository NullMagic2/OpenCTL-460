@echo off
rem Builds the experimental KMDF/VHF package; never installs certificates or changes boot settings.
setlocal
pushd "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "scripts\build_kernel.ps1"
set "result=%ERRORLEVEL%"
popd
exit /b %result%

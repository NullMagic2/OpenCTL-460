@echo off
rem Produces a free developer-signed HID package without installing trust or enabling test mode.
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\test_sign_kernel.ps1"
exit /b %ERRORLEVEL%

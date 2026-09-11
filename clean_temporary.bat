@echo off
rem Delegates cleanup to one PowerShell script with verified project-local paths.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\clean_temporary.ps1" %*
exit /b %ERRORLEVEL%

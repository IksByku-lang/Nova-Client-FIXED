@echo off
setlocal
title Nova Client
cd /d "%~dp0"
rem Cargo is NOT needed to run an already compiled build.
set "EXE=%CD%\launcher\target\release\horizon-launcher.exe"
if not exist "%EXE%" set "EXE=%CD%\horizon-launcher.exe"
if not exist "%EXE%" (
    echo Nova Client has not been built yet.
    echo Run BUILD-NOVA-CLIENT.bat first, or place a compiled horizon-launcher.exe next to this file.
    pause
    exit /b 1
)
rem Keep the Nova fork from attempting an upstream self-update.
set "HORIZON_LAUNCHER_NO_SELF_UPDATE=1"
start "" "%EXE%"
exit /b 0

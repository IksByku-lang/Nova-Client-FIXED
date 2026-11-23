@echo off
rem Points the installed Nova Client launcher at the game build in this folder (Java Bridge and Nova HUD).
set HORIZON_LAUNCHER_NO_SELF_UPDATE=1
set "L=%LOCALAPPDATA%\Programs\Horizon Launcher\horizon-launcher.exe"
if not exist "%L%" (
    echo Nova Client is not installed yet. Run horizon-launcher-setup-NovaClient.exe first.
    pause
    exit /b 1
)
if not exist "%~dp0horizon-client.exe" (
    echo horizon-client.exe is not next to this script.
    pause
    exit /b 1
)
"%L%" java-mods use-build "%~dp0horizon-client.exe"
echo.
echo Done if no error is shown above. Start the game from Nova Client.
pause

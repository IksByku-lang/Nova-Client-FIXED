@echo off
setlocal
 title Nova Client - Build
 cd /d "%~dp0"
 echo ==================================================
 echo                NOVA CLIENT - BUILD
 echo ==================================================
 echo.

 if not exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
     echo Rust/Cargo was not found. Starting the installer...
     call "%~dp0INSTALL-CARGO.bat"
     if errorlevel 1 exit /b 1
 )
 set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

 where python >nul 2>nul
 if errorlevel 1 (
     echo ERROR: Python 3.12 or newer is required. Install it from https://www.python.org/downloads/windows/
     pause
     exit /b 1
 )
 python -c "import sys; raise SystemExit(0 if sys.version_info >= (3,12) else 1)" >nul 2>nul
 if errorlevel 1 (
     echo ERROR: Python 3.12 or newer is required. The Python found on PATH is too old.
     pause
     exit /b 1
 )
 where java >nul 2>nul
 if errorlevel 1 (
     echo ERROR: JDK 25 is required for the Java bridge build. Install Eclipse Temurin 25 from https://adoptium.net/temurin/releases/?version=25
     pause
     exit /b 1
 )
 where javac >nul 2>nul
 if errorlevel 1 (
     echo ERROR: javac was not found. Install the full JDK 25, not only a Java runtime.
     pause
     exit /b 1
 )
 for /f "tokens=3" %%V in ('java -version 2^>^&1 ^| findstr /i "version"') do set "JAVA_VERSION=%%~V"
 for /f "tokens=1 delims=." %%V in ("%JAVA_VERSION%") do set "JAVA_MAJOR=%%V"
 if not "%JAVA_MAJOR%"=="25" (
     echo ERROR: JDK 25 is required. The Java version found on PATH is %JAVA_VERSION%.
     echo Install Eclipse Temurin 25 from https://adoptium.net/temurin/releases/?version=25
     pause
     exit /b 1
 )

 rem This fork must not replace itself with the upstream Horizon launcher.
 set "HORIZON_LAUNCHER_NO_SELF_UPDATE=1"

 set "LOG=%CD%\build-output.log"
 echo Build started %DATE% %TIME% > "%LOG%"
 where link.exe >nul 2>nul
 if errorlevel 1 echo WARNING: link.exe not found in this shell. If the build fails at link time, install the MSVC C++ Build Tools. >> "%LOG%"
 echo Checking Rust and Cargo...
 cargo --version
 rustc --version
 echo.
 echo Building Horizon game with the experimental Java mod bridge...
 cargo build --profile fast --locked -p horizon-client --features horizon-client/java-mods >> "%LOG%" 2>&1
 if errorlevel 1 goto :build_error

 echo.
 echo Building the launcher...
 cargo build --release --locked --manifest-path launcher\Cargo.toml >> "%LOG%" 2>&1
 if errorlevel 1 goto :build_error

 echo.
 rem The installer: the launcher under a name it recognises as the setup (launcher/src/installer.rs).
 if exist "%CD%\dist" rmdir /s /q "%CD%\dist"
 mkdir "%CD%\dist\NovaClient-GameBuild"
 copy /y "%CD%\launcher\target\release\horizon-launcher.exe" "%CD%\dist\horizon-launcher-setup-NovaClient.exe" >nul
 if errorlevel 1 goto :build_error
 copy /y "%CD%\target\fast\horizon-client.exe" "%CD%\dist\NovaClient-GameBuild\horizon-client.exe" >nul
 copy /y "%CD%\packaging\windows\USE-NOVA-GAME-BUILD.bat" "%CD%\dist\NovaClient-GameBuild\USE-NOVA-GAME-BUILD.bat" >nul

 echo.
 echo BUILD SUCCEEDED. Log: %LOG%
 echo.
 echo INSTALLER (double-click to install Nova Client for this account, no administrator):
 echo   %CD%\dist\horizon-launcher-setup-NovaClient.exe
 echo Optional game build with the Java Bridge and the Nova HUD (run USE-NOVA-GAME-BUILD.bat after installing):
 echo   %CD%\dist\NovaClient-GameBuild\
 echo.
 pause
 exit /b 0

:build_error
 echo.
 echo BUILD FAILED. Full output saved to %LOG%
 powershell -NoProfile -Command "Get-Content -Tail 25 '%LOG%'"
 echo On Windows, the Microsoft C++ Build Tools (MSVC) may also be required.
 echo https://visualstudio.microsoft.com/visual-cpp-build-tools/
 pause
 exit /b 1

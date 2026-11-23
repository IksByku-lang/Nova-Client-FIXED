@echo off
setlocal
 title Nova Client - Install Rust and Cargo
 echo ==================================================
 echo             NOVA CLIENT - CARGO SETUP
 echo ==================================================
 echo.

 set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
 if exist "%CARGO_BIN%\cargo.exe" (
     echo Cargo is already installed.
     set "PATH=%CARGO_BIN%;%PATH%"
     cargo --version
     rustc --version
     echo.
     echo Setup is complete. You can close this window.
     pause
     exit /b 0
 )

 echo Downloading the official Rust installer...
 set "RUSTUP_EXE=%TEMP%\rustup-init-nova.exe"
 powershell -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile '%RUSTUP_EXE%'"
 if errorlevel 1 goto :download_error
 if not exist "%RUSTUP_EXE%" goto :download_error

 echo.
 echo Installing Rust stable, Cargo and the Windows MSVC target...
 echo This may take a few minutes. No administrator rights are normally needed.
 "%RUSTUP_EXE%" -y --profile minimal --default-host x86_64-pc-windows-msvc --default-toolchain stable
 if errorlevel 1 goto :install_error

 set "PATH=%CARGO_BIN%;%PATH%"
 if not exist "%CARGO_BIN%\cargo.exe" goto :install_error
 echo.
 echo ==================================================
 echo Rust and Cargo installed successfully!
 echo ==================================================
 cargo --version
 rustc --version
 echo.
 echo You can now run BUILD-NOVA-CLIENT.bat to compile the project.
 pause
 exit /b 0

:download_error
 echo.
 echo ERROR: Could not download rustup from the official Rust server.
 echo Check your internet connection and try again.
 pause
 exit /b 1

:install_error
 echo.
 echo ERROR: Rust/Cargo installation did not complete successfully.
 echo Please try running this file again or visit https://rustup.rs/
 pause
 exit /b 1

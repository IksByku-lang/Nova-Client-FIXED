NOVA CLIENT - AUTOMATIC RUST/CARGO SETUP

1. Double-click INSTALL-CARGO.bat.
2. Wait for the official Rust installer to download and install Rust stable and Cargo.
3. When it says the setup is complete, close the window.
4. To compile Nova Client, double-click BUILD-NOVA-CLIENT.bat.
   BUILD-NOVA-CLIENT.bat also starts INSTALL-CARGO.bat automatically if Cargo is missing.

REQUIREMENTS FOR BUILDING
- Windows 10 or 11, 64-bit.
- Internet connection (the Rust installer and Cargo dependencies are downloaded).
- Python 3.12 or newer.
- JDK 25 for the Java bridge build.
- Microsoft C++ Build Tools (MSVC) with the Windows SDK may be needed for native compilation.

INSTALL-CARGO.bat installs Rust and Cargo only. It does not install Python, Java or Visual Studio Build Tools.

The project may still fail to compile if dependencies, source code, SDKs or the local machine are incompatible.
This script does not claim that a successful build has already been tested.

//! Embeds Nova Client's icon (`packaging/icons/nova.ico`, else Horizon's `horizon.ico`) in the Windows executables, as icon
//! resource 1: Explorer and the task bar show it, and the window takes it as its icon.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Resolve imports before main from System32, never a nearby injector DLL (#13108).
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let nova = format!("{dir}/../packaging/icons/nova.ico");
    let ico = if std::path::Path::new(&nova).is_file() {
        nova
    } else {
        format!("{dir}/../packaging/icons/horizon.ico")
    };
    println!("cargo:rerun-if-changed={ico}");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(&ico);
        // What Explorer's Properties and Task Manager show. The file name stays
        // `horizon-launcher.exe`: the installer and the updater look for it.
        resource.set("ProductName", "Nova Client");
        resource.set("FileDescription", "Nova Client");
        resource.set(
            "LegalCopyright",
            "Nova Client is built on Horizon (MIT licence). See LICENSE and THIRD_PARTY_NOTICES.md.",
        );
        if let Err(e) = resource.compile() {
            panic!("embedding {ico}: {e}");
        }
    }
}

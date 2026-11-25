use std::env;

fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by Cargo");
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|part| part.parse::<u16>().ok())
        .unwrap_or(0);

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/logo.ico");
    resource.set_language(0x0409);
    resource.set("FileDescription", "AnyLink DAP-Link Firmware Flasher");
    resource.set("FileVersion", &version);
    resource.set("InternalName", "anylink");
    resource.set("OriginalFilename", "anylink.exe");
    resource.set("ProductName", "AnyLink");
    resource.set("ProductVersion", &version);
    resource.set_version_info(
        winresource::VersionInfo::FILEVERSION,
        ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16),
    );
    resource.set_version_info(
        winresource::VersionInfo::PRODUCTVERSION,
        ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16),
    );

    resource
        .compile()
        .expect("failed to compile the Windows icon/version resource");
}

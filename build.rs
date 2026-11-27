fn main() {
    println!("cargo:rerun-if-changed=windows/glanceriff.ico");

    // Embeds the icon (and version info) into glanceriff.exe. The build dependency only exists on Windows hosts.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("windows/glanceriff.ico")
            .compile()
            .expect("embedding the Windows icon");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=packaging/windows/keyframe.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("packaging/windows/keyframe.ico")
            .set("ProductName", "Keyframe")
            .set("FileDescription", "Keyframe PDF Presenter")
            .compile()
            .expect("Could not embed the Keyframe Windows icon and version information");
    }
}

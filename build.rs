fn main() {
    println!("cargo:rerun-if-changed=assets/crosshair-reticle.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("Cargo provides target OS");
    if target_os == "windows" {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/crosshair-reticle.ico");
        resource
            .compile()
            .expect("failed to embed the application icon");
    }
}

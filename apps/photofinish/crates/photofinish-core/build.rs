fn main() {
    println!("cargo:rerun-if-changed=src/vision_avatar.m");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        cc::Build::new()
            .file("src/vision_avatar.m")
            .flag("-fobjc-arc")
            .flag("-Wno-deprecated-declarations")
            .compile("photofinish_vision");

        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rustc-link-lib=framework=ImageIO");
        println!("cargo:rustc-link-lib=framework=Vision");
    }
}

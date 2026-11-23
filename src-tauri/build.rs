fn build_macos() {
    let native = std::path::Path::new("src/platform/macos/objc");
    println!("cargo:rerun-if-changed={}", native.display());
    for framework in [
        "Security",
        "LocalAuthentication",
        "AppKit",
        "UniformTypeIdentifiers",
        "ApplicationServices",
        "Carbon",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-lib=curl");
    cc::Build::new()
        .files(
            [
                "host.m",
                "identity.m",
                "hotkeys.m",
                "filing.m",
                "native_ui.m",
            ]
            .map(|file| native.join(file)),
        )
        .flag("-fobjc-arc")
        .flag("-mmacosx-version-min=14.0")
        .compile("jot_host");
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_macos();
    }
    tauri_build::build();
}

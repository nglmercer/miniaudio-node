extern crate napi_build;

fn main() {
    napi_build::setup();

    // Handle platform-specific linking issues
    let target = std::env::var("TARGET").unwrap_or_default();

    // For macOS ARM64, ensure proper linking for NAPI symbols
    if target.contains("apple-darwin") && target.contains("aarch64") {
        println!("cargo:rustc-link-lib=dylib=c++");
        // Use dynamic lookup for NAPI symbols to avoid linking issues
        println!("cargo:rustc-cdylib-link-arg=-Wl,-undefined,dynamic_lookup");
        // Also set the linker flag for cargo
        println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
    }

    // For Linux cross-compilation, handle ALSA dependencies
    if target.contains("unknown-linux") && target.contains("aarch64") {
        // Disable ALSA for cross-compilation to avoid pkg-config issues
        println!("cargo:rustc-cfg=no_alsa");
    }
}

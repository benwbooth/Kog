fn main() {
    // Android loads this shared library at runtime. Fail the build when a
    // vendored static library leaves a symbol unresolved instead of shipping
    // an APK whose native decoder silently fails to load.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg=-Wl,--no-undefined");
    }
}

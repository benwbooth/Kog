//! The frontend built by `crates/kog-web/build.sh` is embedded into this crate
//! with `include_dir!`, and that macro cannot tell cargo to watch the directory.
//! Without this, rebuilding the frontend would not re-embed it: the server would
//! keep serving the previous assets until an unrelated Rust change forced a
//! recompile.
fn main() {
    println!("cargo:rerun-if-changed=web");
}

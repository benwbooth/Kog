//! The bundled C and C++ decoder libraries. This crate has no Rust code: its
//! build script compiles the libraries and tells Cargo to link them, and
//! kog-audio declares the functions it calls. Keeping them in their own crate
//! means only changes under `native/` rebuild them.

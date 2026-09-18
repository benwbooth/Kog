//! Shared foundations for Kog: text encodings, media paths, the sqlite
//! library store, the equalizer, and MPRIS integration.
//!
//! These modules were split out of the monolithic `kog` crate so edits
//! here recompile one small crate instead of the whole application.

pub mod db;
pub mod equalizer;
pub mod media_path;
pub mod mpris;
pub mod text_encoding;

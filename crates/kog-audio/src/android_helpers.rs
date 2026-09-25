//! Resolve packaged helper executables from the Android native library directory.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static HELPER_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

pub fn set_directory(path: PathBuf) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!(
            "Android helper directory is missing: {}",
            path.display()
        ));
    }
    if HELPER_DIRECTORY.get() == Some(&path) {
        return Ok(());
    }
    HELPER_DIRECTORY
        .set(path)
        .map_err(|_| "Android helper directory was already configured".to_owned())
}

pub fn helper_path(name: &str) -> Result<PathBuf, String> {
    let directory: &Path = HELPER_DIRECTORY
        .get()
        .ok_or_else(|| "Android helper directory is not configured".to_owned())?;
    let path = directory.join(format!("lib{name}.so"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "Android audio helper is missing: {}",
            path.display()
        ))
    }
}

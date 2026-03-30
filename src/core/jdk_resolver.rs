use std::path::{Path, PathBuf};

use crate::infra::config::config;

/// Returns all JDK directories found in the configured `jdks_dirs` search paths.
///
/// Each configured directory is scanned for immediate subdirectories; entries
/// that are not directories are silently skipped. This is a pure scan function —
/// no caching is performed here; caching is handled by `memory.rs`.
pub(crate) fn detect_jdks() -> Vec<PathBuf> {
    let config = config();
    let mut jdks = Vec::new();

    for base in &config.jdks_dirs {
        let path = Path::new(base);
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    jdks.push(p);
                }
            }
        }
    }

    jdks
}

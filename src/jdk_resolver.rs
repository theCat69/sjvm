use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::config::config;

static JDKS: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Returns all JDK directories found in the configured `jdks_dirs` search paths.
///
/// Each configured directory is scanned for immediate subdirectories; entries
/// that are not directories are silently skipped. Results are cached in a
/// `OnceLock` after the first call — subsequent calls return the same slice
/// without re-scanning the filesystem.
pub(crate) fn detect_jdks() -> &'static Vec<PathBuf> {
    JDKS.get_or_init(|| {
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
    })
}

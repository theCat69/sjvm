use std::path::{Path, PathBuf};

use crate::infra::config::config;

/// Scans a single directory for immediate subdirectories (JDK candidates).
///
/// Entries that are not directories or cannot be read are silently skipped.
pub(crate) fn scan_dir(path: &Path) -> Vec<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(path) {
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()
    } else {
        vec![]
    }
}

/// Returns all JDK directories found in the configured `jdks_dirs` search paths.
///
/// Each configured directory is scanned for immediate subdirectories; entries
/// that are not directories are silently skipped. This is a pure scan function —
/// no caching is performed here; caching is handled by `memory.rs`.
pub(crate) fn detect_jdks() -> Vec<PathBuf> {
    let config = config();
    let mut jdks = Vec::new();

    for base in &config.jdks_dirs {
        jdks.extend(scan_dir(Path::new(base)));
    }

    jdks.sort();
    jdks
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::scan_dir;

    fn tmp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sjvm_resolver_test_{suffix}_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create test tmp dir");
        dir
    }

    #[test]
    fn test_scan_dir_finds_subdirs() {
        let dir = tmp_dir("scan_subdirs");
        std::fs::create_dir_all(dir.join("jdk-17")).unwrap();
        std::fs::create_dir_all(dir.join("jdk-21")).unwrap();
        let mut result = scan_dir(&dir);
        result.sort();
        assert_eq!(result.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_dir_skips_files() {
        let dir = tmp_dir("scan_files");
        std::fs::create_dir_all(dir.join("jdk-17")).unwrap();
        std::fs::write(dir.join("not-a-dir.txt"), b"file").unwrap();
        let result = scan_dir(&dir);
        assert_eq!(result.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_dir_empty() {
        let dir = tmp_dir("scan_empty");
        let result = scan_dir(&dir);
        assert!(result.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_dir_nonexistent_returns_empty() {
        let dir = PathBuf::from("/nonexistent/sjvm/test/dir");
        let result = scan_dir(&dir);
        assert!(result.is_empty());
    }
}

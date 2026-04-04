# Testable Inner-Loop Extraction Pattern

Demonstrates extracting a pure inner loop from a function that reads global singletons, making it unit-testable without mocking.

## Pattern: `scan_dir` extracted from `detect_jdks`

```rust
// src/core/jdk_resolver.rs

/// Scans a single directory for immediate subdirectories (JDK candidates).
///
/// Pure function — no global state read. Silently skips unreadable entries.
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

/// Calls the pure inner function for each configured dir.
/// Not directly unit-testable (reads config() singleton).
pub(crate) fn detect_jdks() -> Vec<PathBuf> {
    let config = config();
    let mut jdks = Vec::new();
    for base in &config.jdks_dirs {
        jdks.extend(scan_dir(Path::new(base)));
    }
    jdks
}

#[cfg(test)]
mod tests {
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
    fn test_scan_dir_nonexistent_returns_empty() {
        let dir = PathBuf::from("/nonexistent/sjvm/test/dir");
        let result = scan_dir(&dir);
        assert!(result.is_empty()); // never panics
    }
}
```

**Key insight**: When a function reads from a global singleton (like `config()`), extract the testable logic into a separate `pub(crate)` pure function. Test the pure function with real temp dirs; the singleton-reading wrapper stays untested at unit-test level.

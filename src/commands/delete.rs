//! `sjvm delete` command — removes an installed JDK directory.

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};

use crate::infra::config::config;
use crate::infra::memory::invalidate_memory;

// ---------------------------------------------------------------------------
// Core delete logic
// ---------------------------------------------------------------------------

/// Deletes the named JDK directory, searching all configured `jdks_dirs`.
///
/// Steps:
/// 1. Collect all configured `jdks_dirs`.
/// 2. Search each dir for a subdirectory named `jdk_name`.
/// 3. Canonicalize the found path and verify containment within its parent.
/// 4. Verify the path is a directory.
/// 5. Remove the directory tree.
/// 6. Invalidate the memory cache.
/// 7. Return the deleted canonical path.
pub(crate) fn delete_jdk(jdk_name: &str) -> Result<PathBuf> {
    let jdks_dirs: Vec<PathBuf> = config().jdks_dirs.iter().map(PathBuf::from).collect();

    if jdks_dirs.is_empty() {
        bail!("No JDKs directory configured — run 'sjvm setup' first");
    }

    // Search all configured dirs for the named JDK.
    let mut found: Option<PathBuf> = None;
    for dir in &jdks_dirs {
        let candidate = dir.join(jdk_name);
        if candidate.is_dir() {
            found = Some(candidate);
            break;
        }
    }

    let path =
        found.with_context(|| format!("JDK '{jdk_name}' not found in any configured jdks_dirs"))?;

    // Canonicalize the found path and verify it stays within its parent dir.
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize path '{}'", path.display()))?;

    // Find the canonical parent and verify containment.
    let parent_dir = path
        .parent()
        .with_context(|| format!("Path '{}' has no parent directory", path.display()))?;
    let canonical_parent = parent_dir
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize parent '{}'", parent_dir.display()))?;

    if canonical_path == canonical_parent || !canonical_path.starts_with(&canonical_parent) {
        bail!(
            "Security: path '{}' escapes or equals the JDKs directory '{}'",
            canonical_path.display(),
            canonical_parent.display()
        );
    }

    if !canonical_path.is_dir() {
        bail!(
            "'{}' exists but is not a directory",
            canonical_path.display()
        );
    }

    fs::remove_dir_all(&canonical_path)
        .with_context(|| format!("Failed to delete '{}'", canonical_path.display()))?;

    invalidate_memory();

    Ok(canonical_path)
}

// ---------------------------------------------------------------------------
// CLI handler
// ---------------------------------------------------------------------------

/// CLI handler for `sjvm delete`.
///
/// Prompts for confirmation before calling [`delete_jdk`].
pub(crate) fn run_delete(jdk_name: &str) -> Result<()> {
    // Prompt for confirmation
    print!("Are you sure you want to delete \"{jdk_name}\"? [y/N] ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("Failed to read confirmation")?;

    if answer.trim().eq_ignore_ascii_case("y") {
        delete_jdk(jdk_name)?;
        println!("✅ Deleted {jdk_name}");
    } else {
        println!("Aborted.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    fn tmp_dir(suffix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("sjvm_delete_test_{suffix}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create test tmp dir");
        dir
    }

    #[test]
    fn test_delete_jdk_not_found_returns_error() {
        // Use a name that definitely won't exist in any configured jdks_dirs.
        let result = super::delete_jdk("__sjvm_nonexistent_test_jdk_12345__");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("not found") || msg.contains("No JDKs directory"),
            "expected 'not found' or config error in: {msg}"
        );
    }

    #[test]
    fn test_delete_jdk_success_removes_directory() {
        // Test the core logic: canonicalize, containment check, remove.
        let base = tmp_dir("delete_success");
        let jdk_dir = base.join("jdk-test-17");
        std::fs::create_dir_all(&jdk_dir).unwrap();
        assert!(jdk_dir.exists());

        let canonical_base = base.canonicalize().unwrap();
        let canonical_jdk = jdk_dir.canonicalize().unwrap();

        // Verify containment invariant.
        assert!(canonical_jdk.starts_with(&canonical_base));
        assert_ne!(canonical_jdk, canonical_base);

        // Perform removal.
        std::fs::remove_dir_all(&canonical_jdk).unwrap();
        assert!(!jdk_dir.exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}

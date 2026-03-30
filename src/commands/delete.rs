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
// Input validation
// ---------------------------------------------------------------------------

/// Validates a JDK name for use as a directory component.
///
/// Rules: not empty, not `.`, max 128 chars, no `/`, `\`, `..`, or NUL bytes.
pub(crate) fn validate_delete_name(s: &str) -> Result<String, String> {
    if s == "." {
        return Err("JDK name cannot be '.'".to_string());
    }
    if s.is_empty() {
        return Err("JDK name cannot be empty".to_owned());
    }
    if s.len() > 128 {
        return Err("JDK name too long (max 128 chars)".to_owned());
    }
    if s.contains('\0') {
        return Err("JDK name contains a NUL byte which is not allowed".to_owned());
    }
    if s.contains('/') {
        return Err("JDK name must not contain '/'".to_owned());
    }
    if s.contains('\\') {
        return Err("JDK name must not contain '\\'".to_owned());
    }
    if s == ".." || s.contains("/../") || s.ends_with("/..") || s.starts_with("../") {
        return Err("JDK name must not contain path traversal ('..')".to_owned());
    }
    // Simpler check: if the name itself is ".." after splitting on / (already blocked above),
    // also block any name that is exactly ".." or starts with ".."
    if s.starts_with("..") {
        return Err("JDK name must not start with '..'".to_owned());
    }
    Ok(s.to_owned())
}

// ---------------------------------------------------------------------------
// Core delete logic
// ---------------------------------------------------------------------------

/// Deletes the named JDK directory from the first configured `jdks_dirs`.
///
/// Steps:
/// 1. Validate `jdk_name` (no traversal, not empty, max 128 chars).
/// 2. Get `dest_dir` from config.
/// 3. Build and canonicalize-check the target path.
/// 4. Verify the path exists and is a directory.
/// 5. Remove the directory tree.
/// 6. Invalidate the memory cache.
/// 7. Return the deleted path.
pub(crate) fn delete_jdk(jdk_name: &str) -> Result<PathBuf> {
    // Step 1: validate name
    validate_delete_name(jdk_name).map_err(|e| anyhow::anyhow!(e))?;

    // Step 2: get dest_dir
    let dest_dir_str = config()
        .jdks_dirs
        .first()
        .context("No JDKs directory configured — run 'sjvm setup' first")?;
    let dest_dir = PathBuf::from(dest_dir_str);

    // Step 3: build path
    let path = dest_dir.join(jdk_name);

    // Step 4: canonicalize dest_dir and verify path stays inside it
    // Canonicalize only the parent; the target may not exist yet for validation.
    let canonical_dest = dest_dir
        .canonicalize()
        .with_context(|| format!("JDKs directory '{}' does not exist", dest_dir.display()))?;

    // We check the constructed path without canonicalization (the dir may not exist).
    // Strip the jdk_name component and confirm the remainder equals canonical_dest.
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("JDK '{}' not found in '{}'", jdk_name, dest_dir.display()))?;

    if !canonical_path.starts_with(&canonical_dest) {
        bail!(
            "Security: path '{}' escapes the JDKs directory '{}'",
            canonical_path.display(),
            canonical_dest.display()
        );
    }

    // Step 5: verify it is a directory
    if !canonical_path.is_dir() {
        bail!(
            "'{}' exists but is not a directory",
            canonical_path.display()
        );
    }

    // Step 6: remove directory tree
    fs::remove_dir_all(&canonical_path)
        .with_context(|| format!("Failed to delete '{}'", canonical_path.display()))?;

    // Step 7: invalidate cache
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
    // Validate name early for a fast CLI error; all filesystem checks are in delete_jdk.
    validate_delete_name(jdk_name).map_err(|e| anyhow::anyhow!(e))?;

    // Prompt for confirmation
    print!("Are you sure you want to delete \"{jdk_name}\"? [y/N] ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("Failed to read confirmation")?;

    if answer.trim().eq_ignore_ascii_case("y") {
        delete_jdk(jdk_name)?;
        println!("✓ Deleted {jdk_name}");
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
    use super::validate_delete_name;

    #[test]
    fn test_delete_jdk_rejects_path_traversal() {
        let result = validate_delete_name("../etc");
        assert!(result.is_err(), "expected error for '../etc'");
        // The input contains '/' so that check fires first; either way it's rejected
        let msg = result.unwrap_err();
        assert!(
            msg.contains("..") || msg.contains("traversal") || msg.contains("/"),
            "expected rejection reason in error, got: {msg}"
        );
    }

    #[test]
    fn test_delete_jdk_rejects_empty() {
        let result = validate_delete_name("");
        assert!(result.is_err(), "expected error for empty name");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("empty"),
            "expected 'empty' in error, got: {msg}"
        );
    }

    #[test]
    fn test_delete_jdk_rejects_nul() {
        let result = validate_delete_name("jdk\0name");
        assert!(result.is_err(), "expected error for NUL byte");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("NUL") || msg.contains("nul") || msg.contains("null"),
            "expected NUL mention in error, got: {msg}"
        );
    }

    #[test]
    fn test_delete_jdk_rejects_forward_slash() {
        let result = validate_delete_name("foo/bar");
        assert!(result.is_err(), "expected error for '/'");
    }

    #[test]
    fn test_delete_jdk_rejects_backslash() {
        let result = validate_delete_name("foo\\bar");
        assert!(result.is_err(), "expected error for '\\'");
    }

    #[test]
    fn test_delete_jdk_rejects_too_long() {
        let long = "a".repeat(129);
        let result = validate_delete_name(&long);
        assert!(result.is_err(), "expected error for name > 128 chars");
    }

    #[test]
    fn test_delete_jdk_accepts_valid_name() {
        for name in &["jdk-21", "temurin-17", "graalvm-ce-java17", "jdk21"] {
            assert!(
                validate_delete_name(name).is_ok(),
                "expected ok for '{name}'"
            );
        }
    }

    #[test]
    fn test_delete_jdk_accepts_exactly_128_chars() {
        let exactly = "a".repeat(128);
        assert!(validate_delete_name(&exactly).is_ok());
    }

    #[test]
    fn test_delete_jdk_rejects_dotdot_alone() {
        let result = validate_delete_name("..");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_jdk_rejects_single_dot() {
        let result = validate_delete_name(".");
        assert!(result.is_err(), "expected error for '.'");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("'.'"),
            "expected message to mention '.', got: {msg}"
        );
    }
}

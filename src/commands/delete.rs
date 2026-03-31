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

/// Deletes the named JDK directory from the first configured `jdks_dirs`.
///
/// Steps:
/// 1. Get `dest_dir` from config.
/// 2. Build and canonicalize-check the target path.
/// 3. Verify the path exists and is a directory.
/// 4. Remove the directory tree.
/// 5. Invalidate the memory cache.
/// 6. Return the deleted path.
pub(crate) fn delete_jdk(jdk_name: &str) -> Result<PathBuf> {
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

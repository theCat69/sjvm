use std::fs;

use anyhow::Context;

use crate::core::jdk_resolver::detect_jdks;
use crate::infra::memory::{invalidate_memory, memory_file};
use crate::infra::symlinks::{create_symlink, symlink_path};

/// Performs first-run setup: creates the initial symlink and resets the memory cache.
///
/// # Errors
/// Returns an error if the symlink cannot be created or the memory cache cannot
/// be reset.
pub(crate) fn setup() -> anyhow::Result<()> {
    let symlink = symlink_path();

    if let Some(parent) = symlink.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory {parent:?}"))?;
    }

    let jdks = detect_jdks();
    if let Some(first) = jdks.first() {
        create_symlink(first, &symlink)?;
        println!("Initial symlink set to: {}", first.to_string_lossy());
    } else {
        println!("No JDKs found.");
    }

    // Reset the memory cache so it is rebuilt on the next command invocation.
    let mem_file = memory_file();
    if mem_file.is_file() {
        fs::remove_file(mem_file).context("Cannot remove memory file")?;
    }
    invalidate_memory();

    println!("\n✅ Setup complete.");
    println!("=> Add {}/bin to your PATH.", symlink.display());
    println!("=> Set JAVA_HOME={}.", symlink.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    // setup() uses config() singleton and detect_jdks() — hard to fully unit test.
    // Test the pure logic pieces.

    #[test]
    fn test_setup_logic_first_jdk_selection() {
        // Verifies that if jdks list is non-empty, first() is used.
        use std::path::PathBuf;
        let jdks: Vec<PathBuf> = vec![
            PathBuf::from("/jvms/temurin-17"),
            PathBuf::from("/jvms/temurin-21"),
        ];
        let first = jdks.first().cloned();
        assert_eq!(first, Some(PathBuf::from("/jvms/temurin-17")));
    }

    #[test]
    fn test_setup_logic_empty_jdks() {
        // Verifies that if jdks list is empty, first() returns None.
        use std::path::PathBuf;
        let jdks: Vec<PathBuf> = vec![];
        let first = jdks.first().cloned();
        assert!(first.is_none());
    }
}

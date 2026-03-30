use std::fs;

use anyhow::Context;

use crate::core::jdk_resolver::detect_jdks;
use crate::infra::memory::{memory, memory_file};
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

    // Reset the memory cache so it is rebuilt from the new symlink target.
    // NOTE: memory() uses OnceLock — removing the cache file and calling
    // memory() here causes a fresh rebuild only on first run per process.
    // The static is NOT re-initialised on subsequent calls within the same
    // process, so this call is for the side-effect of flushing the file cache.
    let mem_file = memory_file();
    if mem_file.is_file() {
        fs::remove_file(mem_file).context("Cannot remove memory file")?;
    }
    let _ = memory();

    println!("\n✅ Setup complete.");
    println!("=> Add {}/bin to your PATH.", symlink.display());
    println!("=> Set JAVA_HOME={}.", symlink.display());

    Ok(())
}

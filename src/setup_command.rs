use std::fs;

use anyhow::Context;

use crate::{
    jdk_resolver::detect_jdks,
    memory::{memory, memory_file},
    symlinks::{create_symlink, symlink_path},
};

/// Performs first-run setup: creates the initial symlink and resets the memory cache.
///
/// # Errors
/// Returns an error if the symlink cannot be created or the memory cache cannot
/// be reset.
pub(crate) fn setup() -> anyhow::Result<()> {
    let symlink = symlink_path();

    if let Some(parent) = symlink.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent directory {:?}", parent))?;
    }

    let jdks = detect_jdks();
    if let Some(first) = jdks.first() {
        create_symlink(first, &symlink)?;
        println!("Initial symlink set to: {}", first.to_string_lossy());
    } else {
        println!("No JDKs found.");
    }

    // Reset the memory cache so it is rebuilt from the new symlink target.
    let mem_file = memory_file();
    if mem_file.is_file() {
        fs::remove_file(mem_file).context("Cannot remove memory file")?;
    }
    let _ = memory();

    println!("\n✅ Setup complete.");
    if cfg!(target_os = "windows") {
        println!("=> Add C:\\Java\\current\\bin to your PATH.");
        println!("=> Add C:\\Java\\current as your JAVA_HOME.");
    } else {
        println!("=> Add $HOME/.java/current/bin to your PATH.");
        println!("=> Add $HOME/.java/current as your JAVA_HOME.");
    }

    Ok(())
}

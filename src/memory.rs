use anyhow::{Context, bail};
use bincode::{Decode, Encode, config};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{app_dirs::app_dirs, jdk_resolver::detect_jdks, symlinks::symlink_path};

static MEMORY: OnceLock<Memory> = OnceLock::new();
static MEMORY_FILE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Encode, Decode, PartialEq, Debug)]
pub(crate) struct Memory {
    pub(crate) current: PathBuf,
    pub(crate) jdks: Vec<PathBuf>,
}

/// Returns a reference to the in-memory JDK cache, initialising it on first call.
///
/// # Errors
/// Panics at startup (via `.expect`) if the cache file cannot be read or
/// written; this is intentional — the binary cannot function without it.
pub(crate) fn memory() -> &'static Memory {
    MEMORY.get_or_init(|| lazy_init_memory().expect("Failed to initialise JDK memory cache"))
}

/// Returns the path to the persistent memory cache file.
pub(crate) fn memory_file() -> &'static PathBuf {
    MEMORY_FILE.get_or_init(|| Path::join(&app_dirs().data_dir, "sjvm-mem"))
}

fn lazy_init_memory() -> anyhow::Result<Memory> {
    let mem_file = memory_file();
    if !mem_file.is_file() {
        let current = current_jdk()?;
        let jdks = detect_jdks();
        let memory = Memory {
            current: current.to_path_buf(),
            jdks: jdks.to_owned(),
        };
        dump_binaries(&memory)?;
        Ok(memory)
    } else {
        load_from_binaries()
    }
}

fn dump_binaries(memory: &Memory) -> anyhow::Result<()> {
    fs::write(
        memory_file(),
        bincode::encode_to_vec(memory, config::standard())
            .context("Cannot encode memory to binaries")?,
    )
    .context("Cannot write to memory file")?;
    Ok(())
}

fn load_from_binaries() -> anyhow::Result<Memory> {
    let file = fs::read(memory_file()).context("Cannot read memory file")?;
    let (decoded, _): (Memory, usize) = bincode::decode_from_slice(&file, config::standard())
        .context("Cannot decode binaries from memory file")?;
    Ok(decoded)
}

fn current_jdk() -> anyhow::Result<&'static PathBuf> {
    let current_link = symlink_path();
    let current = std::fs::read_link(&current_link)
        .with_context(|| format!("Cannot read symlink '{}'", current_link.display()))?;
    for jdk in detect_jdks() {
        if jdk == &current {
            return Ok(jdk);
        }
    }
    bail!(
        "Active JDK '{}' is not in any configured jdks_dirs. Run 'sjvm setup' or add its parent directory to your config.",
        current_link.display()
    )
}

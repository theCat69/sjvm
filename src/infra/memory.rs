use anyhow::{bail, Context};
use bincode::{config, Decode, Encode};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::core::jdk_resolver::detect_jdks;
use crate::infra::app_dirs::app_dirs;
use crate::infra::config::config as app_config;
use crate::infra::symlinks::symlink_path;

static MEMORY: OnceLock<Memory> = OnceLock::new();
static MEMORY_FILE: OnceLock<PathBuf> = OnceLock::new();

/// In-process cache of the JDK list and the currently active JDK.
///
/// Serialised to disk at `sjvm-mem` using [bincode] for fast startup.
#[derive(Encode, Decode, PartialEq, Debug)]
pub(crate) struct Memory {
    /// Path to the JDK that is currently selected (the symlink target).
    pub(crate) current: PathBuf,
    /// All JDK directories discovered in the configured `jdks_dirs`.
    pub(crate) jdks: Vec<PathBuf>,
}

/// Returns a reference to the in-memory JDK cache, initialising it on first call.
///
/// # Panics
/// Panics at startup if the cache file cannot be read or written; this is
/// intentional — the binary cannot function without a valid cache.
pub(crate) fn memory() -> &'static Memory {
    MEMORY.get_or_init(|| lazy_init_memory().expect("Failed to initialise JDK memory cache"))
}

/// Returns the path to the persistent memory cache file.
///
/// The file is located in the platform-specific data directory
/// (e.g. `~/.local/share/sjvm/sjvm-mem` on Linux).
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

fn validate_cached_memory(memory: Memory) -> anyhow::Result<Memory> {
    let cfg = app_config();
    let jdks_dirs: Vec<PathBuf> = cfg.jdks_dirs.iter().map(PathBuf::from).collect();

    // Filter out stale entries (directory was removed) and warn.
    let valid_jdks: Vec<PathBuf> = memory
        .jdks
        .into_iter()
        .filter(|jdk| {
            if !jdk.is_dir() {
                eprintln!("sjvm: cached JDK no longer exists: {}", jdk.display());
                return false;
            }
            // Validate the cached path is still inside a configured jdks_dir.
            let in_configured_dir = jdks_dirs.iter().any(|d| jdk.starts_with(d));
            if !in_configured_dir {
                eprintln!(
                    "sjvm: cached JDK '{}' is outside all configured jdks_dirs — removing from cache",
                    jdk.display()
                );
            }
            in_configured_dir
        })
        .collect();

    // Validate that the cached current JDK is a real directory.
    if !memory.current.as_os_str().is_empty() && !memory.current.is_dir() {
        bail!(
            "Cached current JDK '{}' no longer exists. Run 'sjvm setup' to rebuild the cache.",
            memory.current.display()
        );
    }

    Ok(Memory {
        current: memory.current,
        jdks: valid_jdks,
    })
}

fn load_from_binaries() -> anyhow::Result<Memory> {
    let file = fs::read(memory_file()).context("Cannot read memory file")?;
    let (decoded, _): (Memory, usize) = bincode::decode_from_slice(&file, config::standard())
        .context("Cannot decode binaries from memory file")?;
    validate_cached_memory(decoded)
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bincode::config;

    use super::Memory;

    /// Verifies that a `Memory` value survives a bincode encode → decode round-trip
    /// with all fields intact.
    #[test]
    fn test_memory_bincode_round_trip() {
        let original = Memory {
            current: PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            jdks: vec![
                PathBuf::from("/usr/lib/jvm/temurin-11-jdk"),
                PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
                PathBuf::from("/usr/lib/jvm/temurin-21-jdk"),
            ],
        };

        let encoded =
            bincode::encode_to_vec(&original, config::standard()).expect("encode should succeed");

        let (decoded, bytes_consumed): (Memory, usize) =
            bincode::decode_from_slice(&encoded, config::standard())
                .expect("decode should succeed");

        assert_eq!(decoded, original);
        assert_eq!(
            bytes_consumed,
            encoded.len(),
            "all bytes should be consumed"
        );
    }

    /// Verifies that an empty JDK list round-trips correctly.
    #[test]
    fn test_memory_bincode_round_trip_empty_jdks() {
        let original = Memory {
            current: PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            jdks: vec![],
        };

        let encoded =
            bincode::encode_to_vec(&original, config::standard()).expect("encode should succeed");

        let (decoded, _): (Memory, usize) =
            bincode::decode_from_slice(&encoded, config::standard())
                .expect("decode should succeed");

        assert_eq!(decoded, original);
    }

    /// Verifies that corrupted bytes produce a decode error rather than silent
    /// data corruption.
    #[test]
    fn test_memory_bincode_rejects_corrupted_bytes() {
        let corrupted = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00];
        let result: Result<(Memory, usize), _> =
            bincode::decode_from_slice(&corrupted, config::standard());
        assert!(
            result.is_err(),
            "corrupted bytes should not decode successfully"
        );
    }
}

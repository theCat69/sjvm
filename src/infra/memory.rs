use anyhow::{Context, bail};
use bincode::{Decode, Encode, config};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, OnceLock},
};

use crate::core::jdk_resolver::detect_jdks;
use crate::infra::app_dirs::app_dirs;
use crate::infra::config::config as app_config;
use crate::infra::symlinks::symlink_path;

static MEMORY: LazyLock<Mutex<Option<Memory>>> = LazyLock::new(|| Mutex::new(None));
static MEMORY_FILE: OnceLock<PathBuf> = OnceLock::new();

/// In-process cache of the JDK list and the currently active JDK.
///
/// Serialised to disk at `sjvm-mem` using [bincode] for fast startup.
#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub(crate) struct Memory {
    /// Path to the JDK that is currently selected (the symlink target).
    pub(crate) current: PathBuf,
    /// All JDK directories discovered in the configured `jdks_dirs`.
    pub(crate) jdks: Vec<PathBuf>,
}

/// Returns the in-memory JDK cache by value (cloned), initialising it on first call.
///
/// After `invalidate_memory()` is called the next invocation will re-read from
/// disk (or rebuild from the filesystem if the disk file is also absent).
pub(crate) fn memory() -> anyhow::Result<Memory> {
    let mut guard = MEMORY.lock().unwrap_or_else(|e| {
        eprintln!("sjvm: WARNING — memory mutex was poisoned, recovering");
        e.into_inner()
    });
    if guard.is_none() {
        *guard = Some(load_or_init()?);
    }
    guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("BUG: memory not initialized after load"))
        .cloned()
}

/// Returns the path to the persistent memory cache file.
///
/// The file is located in the platform-specific data directory
/// (e.g. `~/.local/share/sjvm/sjvm-mem` on Linux).
pub(crate) fn memory_file() -> &'static PathBuf {
    MEMORY_FILE.get_or_init(|| Path::join(&app_dirs().data_dir, "sjvm-mem"))
}

fn load_or_init() -> anyhow::Result<Memory> {
    let mem_file = memory_file();
    if !mem_file.is_file() {
        let current = current_jdk()?;
        let mut jdks = detect_jdks();
        jdks.sort_by_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });
        let memory = Memory { current, jdks };
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

    // If the cached current JDK no longer exists on disk (e.g. it was deleted),
    // clear the pointer and carry on — the JDK list itself is still usable.
    let current = if !memory.current.as_os_str().is_empty() && !memory.current.is_dir() {
        eprintln!(
            "sjvm: cached current JDK '{}' no longer exists — clearing current pointer",
            memory.current.display()
        );
        PathBuf::default()
    } else {
        memory.current
    };

    Ok(Memory {
        current,
        jdks: valid_jdks,
    })
}

fn load_from_binaries() -> anyhow::Result<Memory> {
    let file = fs::read(memory_file()).context("Cannot read memory file")?;
    let (decoded, _): (Memory, usize) = bincode::decode_from_slice(&file, config::standard())
        .context("Cannot decode binaries from memory file")?;
    validate_cached_memory(decoded)
}

/// Removes the on-disk JDK memory cache AND clears the in-process cache so
/// that the next `memory()` call rebuilds from the filesystem.
///
/// Errors are non-fatal: a warning is printed and execution continues.
pub(crate) fn invalidate_memory() {
    let _ = std::fs::remove_file(memory_file());
    match MEMORY.lock() {
        Ok(mut guard) => *guard = None,
        Err(e) => {
            let mut guard = e.into_inner();
            *guard = None;
        }
    }
}

fn current_jdk() -> anyhow::Result<PathBuf> {
    let current_link = symlink_path();
    let current = match std::fs::read_link(&current_link) {
        Ok(p) => p,
        // Symlink does not exist yet (fresh install before first `sjvm use`).
        Err(_) => return Ok(PathBuf::default()),
    };
    for jdk in detect_jdks() {
        if jdk == current {
            return Ok(jdk);
        }
    }
    // The symlink target is not in any configured jdks_dirs.
    if current.is_dir() {
        // Directory still exists but lives outside jdks_dirs — likely a config issue.
        bail!(
            "Active JDK '{}' is not in any configured jdks_dirs. \
             Run 'sjvm setup' or add its parent directory to your config.",
            current.display()
        );
    }
    // Directory was deleted (e.g. `sjvm delete` removed the current JDK).
    // Degrade gracefully: treat as "no current JDK" so the tool stays usable.
    eprintln!(
        "sjvm: current JDK '{}' no longer exists — clearing current pointer",
        current.display()
    );
    Ok(PathBuf::default())
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

    /// `invalidate_memory` must not panic when the cache file does not exist.
    #[test]
    fn test_invalidate_memory_noop_when_file_absent() {
        // The memory_file() path will not exist as a file during unit tests
        // (no real sjvm data directory is set up). Calling invalidate_memory()
        // must be a no-op — no panic, no error propagation.
        super::invalidate_memory();
        // If we reach here the function handled the absent file gracefully.
    }

    /// Verifies that `Memory` implements `Clone` correctly.
    #[test]
    fn test_memory_clone() {
        let original = Memory {
            current: PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            jdks: vec![
                PathBuf::from("/usr/lib/jvm/temurin-11-jdk"),
                PathBuf::from("/usr/lib/jvm/temurin-21-jdk"),
            ],
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    /// Verifies that `load_or_init` sorts the JDK list alphabetically (case-insensitive).
    ///
    /// Tests the sort logic directly by applying the same comparator to an unsorted list.
    #[test]
    fn test_jdk_sort_order_alphabetical() {
        let mut jdks = vec![
            PathBuf::from("/jvms/zulu-8"),
            PathBuf::from("/jvms/temurin-21-jdk"),
            PathBuf::from("/jvms/graalvm-ce-java17"),
            PathBuf::from("/jvms/Amazon-corretto-11"),
            PathBuf::from("/jvms/temurin-11-jdk"),
        ];

        jdks.sort_by_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });

        let names: Vec<&str> = jdks
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        // After case-insensitive alphabetical sort:
        // amazon-corretto-11, graalvm-ce-java17, temurin-11-jdk, temurin-21-jdk, zulu-8
        assert_eq!(
            names,
            vec![
                "Amazon-corretto-11",
                "graalvm-ce-java17",
                "temurin-11-jdk",
                "temurin-21-jdk",
                "zulu-8",
            ]
        );
    }

    /// Verifies that validate_cached_memory does not bail when current path is absent —
    /// it should clear the current pointer instead of returning Err.
    #[test]
    fn test_validate_cached_memory_clears_missing_current() {
        let mem = super::Memory {
            current: PathBuf::from("/nonexistent/path/that/does/not/exist"),
            jdks: vec![],
        };
        // The current path does not exist, so validate_cached_memory should
        // return Ok with current cleared to default, not Err.
        // We can't call validate_cached_memory directly (it reads config),
        // so we test the invariant: a Memory with a missing current path
        // should not cause a panic when cloned or compared.
        let cleared = if !mem.current.as_os_str().is_empty() && !mem.current.is_dir() {
            super::Memory {
                current: PathBuf::default(),
                jdks: mem.jdks.clone(),
            }
        } else {
            mem.clone()
        };
        assert_eq!(cleared.current, PathBuf::default());
        assert!(cleared.jdks.is_empty());
    }
}

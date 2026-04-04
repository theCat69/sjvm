<!-- Demonstrates: bincode 2.0 Encode/Decode derive; encode_to_vec / decode_from_slice; round-trip test pattern -->

```rust
use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use std::{fs, path::{Path, PathBuf}};

// ---------------------------------------------------------------------------
// bincode 2.0 derive pattern
//
// - Derive both Encode AND Decode on the same struct.
// - Also derive Debug, Clone, PartialEq — needed for tests and cache validation.
// - PathBuf is supported by bincode 2.0 (implements Encode/Decode).
// ---------------------------------------------------------------------------

#[derive(Encode, Decode, PartialEq, Debug, Clone)]
pub(crate) struct Memory {
    /// Path to the currently selected JDK (symlink target).
    pub(crate) current: PathBuf,
    /// All JDK directories discovered in jdks_dirs.
    pub(crate) jdks: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Writes a Memory value to the binary cache file on disk.
fn dump_binaries(memory: &Memory, dest: &Path) -> Result<()> {
    // config::standard() = little-endian, varint lengths, no length limit
    let bytes = bincode::encode_to_vec(memory, config::standard())
        .context("Cannot encode memory to binaries")?;
    fs::write(dest, bytes).context("Cannot write to memory file")?;
    Ok(())
}

/// Reads the binary cache file and decodes it back into a Memory value.
fn load_from_binaries(src: &Path) -> Result<Memory> {
    let file = fs::read(src).context("Cannot read memory file")?;
    // decode_from_slice returns (T, bytes_consumed) — discard the byte count
    let (decoded, _): (Memory, usize) = bincode::decode_from_slice(&file, config::standard())
        .context("Cannot decode binaries from memory file")?;
    Ok(decoded)
}

// --- Tests ------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use bincode::config;

    #[test]
    fn test_memory_bincode_round_trip() {
        let original = Memory {
            current: PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            jdks: vec![
                PathBuf::from("/usr/lib/jvm/temurin-11-jdk"),
                PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            ],
        };

        let encoded =
            bincode::encode_to_vec(&original, config::standard()).expect("encode should succeed");

        let (decoded, bytes_consumed): (Memory, usize) =
            bincode::decode_from_slice(&encoded, config::standard())
                .expect("decode should succeed");

        assert_eq!(decoded, original);
        // Verify all bytes were consumed (no trailing garbage)
        assert_eq!(bytes_consumed, encoded.len(), "all bytes should be consumed");
    }

    #[test]
    fn test_memory_bincode_rejects_corrupted_bytes() {
        let corrupted = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00];
        let result: Result<(Memory, usize), _> =
            bincode::decode_from_slice(&corrupted, config::standard());
        assert!(result.is_err(), "corrupted bytes should not decode successfully");
    }
}
```

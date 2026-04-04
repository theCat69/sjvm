<!-- Demonstrates: OnceLock<T> for lazy-initialized global singletons; LazyLock<Mutex<Option<T>>> for invalidatable cache -->

```rust
use std::sync::{LazyLock, Mutex, OnceLock};
use std::path::PathBuf;
use anyhow::Result;

// ---------------------------------------------------------------------------
// Pattern 1: OnceLock<T> — immutable singleton, initialized exactly once
//
// Use when: the value never needs to be reset after initialization (config, dirs, HTTP clients).
// ---------------------------------------------------------------------------

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns the global Config, initializing it from disk on the first call.
///
/// # Panics
/// Panics at startup if the config file exists but cannot be parsed; this is
/// intentional — a corrupted config is a fatal error.
pub(crate) fn config() -> &'static Config {
    // get_or_init runs the closure exactly once; concurrent callers block until done.
    CONFIG.get_or_init(|| init_config().expect("Failed to load configuration"))
}

// ---------------------------------------------------------------------------
// Pattern 2: LazyLock<Mutex<Option<T>>> — mutable/invalidatable cached state
//
// Use when: the cached value can be invalidated and must be rebuilt (e.g. JDK memory cache).
// LazyLock is used instead of OnceLock because Mutex<Option<T>> needs initialization itself.
// ---------------------------------------------------------------------------

static MEMORY: LazyLock<Mutex<Option<Memory>>> = LazyLock::new(|| Mutex::new(None));

/// Returns the in-memory JDK cache by value (cloned), initializing on first call.
/// After `invalidate_memory()` is called, the next call rebuilds from disk/fs.
pub(crate) fn memory() -> Result<Memory> {
    // unwrap_or_else(|e| e.into_inner()) = poison recovery pattern (acceptable in non-test code)
    let mut guard = MEMORY.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(load_or_init()?);
    }
    guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("BUG: memory not initialized after load"))
        .cloned()
}

/// Clears both the on-disk cache file and the in-process cache.
/// The next `memory()` call will rebuild from the filesystem.
pub(crate) fn invalidate_memory() {
    // Non-fatal: remove_file returns Err if the file was already gone; that's fine.
    let _ = std::fs::remove_file(memory_file());
    match MEMORY.lock() {
        Ok(mut guard) => *guard = None,
        Err(e) => {
            let mut guard = e.into_inner(); // poison recovery
            *guard = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Pattern 3: OnceLock inside a function — scoped singleton (HTTP client example)
// ---------------------------------------------------------------------------

fn short_client() -> Result<&'static reqwest::blocking::Client> {
    static SHORT_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    // Fast path: already initialized
    if let Some(c) = SHORT_CLIENT.get() {
        return Ok(c);
    }
    let client = build_client()?.build().context("Failed to build HTTP client")?;
    // get_or_init handles race: whichever thread wins, both return the same client
    Ok(SHORT_CLIENT.get_or_init(|| client))
}
```

---
name: project-test
description: Project-specific testing guidelines, test framework conventions, patterns, and coverage requirements
---

# Project Test Guidelines — sjvm

**sjvm** uses Rust's built-in test framework. No external test framework is required.

---

## Test Framework

- **Built-in Rust test framework** (`#[test]`, `assert_eq!`, `assert!`, etc.)
- **anyhow** for `Result`-returning test functions — allows `?` in tests.
- **ratatui `TestBackend`** for TUI widget tests (when `ui` feature is enabled).
- **Docker** for E2E integration tests only (not part of standard development).

---

## Test Location & File Naming

| Test type | Location | Notes |
|-----------|----------|-------|
| Unit tests | Inside `src/**/*.rs` in `#[cfg(test)] mod tests { ... }` | Co-located with code under test |
| Integration / E2E tests | `tests/e2e.rs` | All `#[ignore]`; Docker-only; separate binary |

- Each source file with testable logic should have a `#[cfg(test)] mod tests { ... }` at the bottom.
- Test function names use `test_<behavior_under_test>` in snake_case:
  - `test_find_jdk_by_version_returns_none_when_missing`
  - `test_parse_adoptium_response_valid`
  - `test_validate_version_string_rejects_metacharacters`

---

## Writing Tests

### Standard unit test structure

```rust
#[cfg(test)]
mod tests {
    use super::*;  // access private items from parent module

    #[test]
    fn test_validate_version_string_rejects_empty() {
        assert!(validate_version_string("").is_err());
    }

    // Use anyhow::Result<()> as return type to use ? in tests
    #[test]
    fn test_parse_adoptium_response_valid() -> anyhow::Result<()> {
        let json = serde_json::json!([{ "binary": { "package": {
            "link": "https://example.com/jdk-21.tar.gz",
            "name": "jdk-21.tar.gz"
        }}}]);
        let artifact = parse_adoptium_response(&json, 21)?;
        assert_eq!(artifact.version, 21);
        Ok(())
    }
}
```

### Testing pure functions (preferred pattern)

Design business logic as pure functions accepting explicit parameters — **never** reading from global singletons. The HTTP-calling wrapper is a thin shell around the pure function:

```rust
// Testable pure function: accepts explicit data
pub(crate) fn parse_adoptium_response(json: &Value, version: u16) -> Result<ArtifactInfo> { ... }

// Testable pure function: accepts explicit list
pub(crate) fn find_jdk_by_version_in_list(version: &str, jdks: &[PathBuf], vendor: Option<&Vendor>) -> Vec<PathBuf> { ... }

// NOT directly testable (reads global OnceLock)
pub(crate) fn find_jdk_by_version(version: &str, vendor: Option<&Vendor>) -> Result<Vec<PathBuf>> { ... }
```

### Testing clap CLI parsing

**Always use `try_parse_from` — never `Cli::parse()`** in tests:

```rust
#[test]
fn test_install_command_parses_default_vendor() {
    let cli = Cli::try_parse_from(["sjvm", "install", "21"]).expect("should parse");
    if let Commands::Install { version, force, .. } = cli.command {
        assert_eq!(version, "21");
        assert!(!force);
    } else {
        panic!("expected Commands::Install");
    }
}

#[test]
fn test_install_command_rejects_unknown_vendor() {
    let result = Cli::try_parse_from(["sjvm", "install", "21", "--vendor", "zulu"]);
    assert!(result.is_err());
}
```

### Testing error messages

Assert on error message content when the message text matters:

```rust
#[test]
fn test_error_message_contains_field_name() {
    let result = parse_adoptium_response(&json!([]), 21);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("no JDK builds"),
        "expected 'no JDK builds' in error, got: {msg}"
    );
}
```

### Testing TUI widgets (ratatui)

Use `TestBackend` — renders widgets without a real terminal:

```rust
#[cfg(feature = "ui")]
#[test]
fn test_renders_jdk_list() -> anyhow::Result<()> {
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render_switch_screen(f, f.area(), &state))?;

    let buffer = terminal.backend().buffer().clone();
    // Assert on cell content or snapshot with insta
    assert!(buffer.content().iter().any(|c| c.symbol() == "17"));
    Ok(())
}
```

Note: `TestBackend` uses `core::convert::Infallible` as its error type in ratatui 0.30+.

### Parametric tests (avoid repetition)

```rust
#[test]
fn test_validate_version_string_rejects_metacharacters() {
    for bad in &["17;rm", "17$HOME", "17`id`", "17|cat", "17>out", "17("] {
        assert!(
            validate_version_string(bad).is_err(),
            "expected error for '{bad}'"
        );
    }
}
```

### Platform-conditional tests

```rust
#[cfg(target_os = "linux")]
#[test]
fn test_detect_os_linux() {
    let result = detect_os().expect("detect_os should succeed on linux");
    assert_eq!(result, "linux");
}
```

---

## Mocking & Fixtures

- **No mocking framework**. Design functions to accept injected parameters so tests pass data directly.
- **Test config**: use `merge_config(serde_json::json!({...}))` to test config parsing directly.
- **Filesystem tests**: use `std::env::temp_dir()` for temporary files; clean up in the test.
- **Avoid global state in tests**: do not rely on `OnceLock` singletons — call pure functions or pass data explicitly.
- **JSON fixtures**: use `serde_json::json!({ ... })` macro for inline API response fixtures.

---

## Coverage Requirements

No enforced coverage threshold. Focus coverage efforts on:

- `core/jdk_switcher.rs` — pure functions with high testability.
- `infra/config.rs` — config parsing, merging, and path validation.
- `core/jdk_catalog.rs` — parse helpers (pure functions, fully unit-testable).
- `commands/mod.rs` — validate_version_string.

Coverage tools:
- **Linux**: `cargo tarpaulin --out Html`
- **Cross-platform**: `cargo llvm-cov`

---

## Running Tests

### Unit tests (standard development — always use this)

```bash
# Preferred (MCP tools)
rust-mcp-server_cargo-test                        # Run all unit tests
rust-mcp-server_cargo-test --testname test_name   # Run specific test by name

# Alternative (standard Cargo)
cargo test --all-features                  # Main local/CI validation path
cargo test                                 # Without features
cargo test test_name                       # Run tests matching substring
cargo test --features ui                   # Test with TUI feature enabled
```

### E2E tests (Docker only — run ONLY when specifically requested)

**Never run E2E tests directly.** They require Docker with Ubuntu 22.04, Java 11, 17, and 21:

```bash
docker compose -f ./docker/it-ubuntu-compose.yaml up --build
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

Inside Docker, E2E tests run as:
```bash
cargo test --all-features --test e2e test_setup -- --ignored
cargo test --all-features --test e2e -- --skip test_setup --ignored --test-threads=1
```

---

## CI Test Commands

```bash
cargo fmt --check                              # Fail on formatting diff
cargo check --all-features                     # Fail on type errors
cargo test --all-features                      # Fail on test failures
cargo clippy --all-features -- -D warnings     # Fail on lint warnings
cargo +1.88 check --all-features               # MSRV compatibility
cargo +1.88 test --all-features
cargo audit                                    # Fail on known vulnerabilities
```

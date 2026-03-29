# Testing Guidelines

Test conventions and strategies for **sjvm** — a Rust 2024 edition CLI binary.

---

## Test Framework

- **Built-in Rust test framework** (`#[test]`, `assert_eq!`, etc.) — no external test framework needed.
- **anyhow** for `Result`-returning test functions.
- **ratatui `TestBackend`** for TUI widget tests (when `ui` feature is enabled).
- **Docker** for E2E integration tests (see below).

---

## Test Location & File Naming

| Test type | Location | Notes |
|-----------|----------|-------|
| Unit tests | Inside `src/*.rs` in `#[cfg(test)] mod tests { ... }` | Co-located with the code under test |
| Integration / E2E tests | `tests/e2e.rs` | Separate test crate; Docker-only |

- Each source file with testable logic should have a `#[cfg(test)]` module at the bottom.
- Integration test files in `tests/` are compiled as separate crates — no `#[cfg(test)]` wrapper needed there.
- Test function names use the pattern `test_<what_it_tests>`, e.g. `test_find_jdk_by_version_returns_none_when_missing`.

---

## Writing Tests

### Standard unit test structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_find_jdk_by_version_returns_correct_entry() -> Result<()> {
        let jdks = vec![/* ... */];
        let result = find_jdk_by_version_in_list("17", &jdks);
        assert!(matches!(result, JdkLookupResult::Found(_)));
        Ok(())
    }
}
```

Key rules:
- Use `use super::*;` to access the parent module's items.
- Prefer `-> anyhow::Result<()>` return type to use `?` in tests — avoids `.unwrap()`.
- Prefer `assert_eq!` over `assert!` for better failure messages.
- Use `#[should_panic(expected = "msg")]` for expected-panic tests (rare — prefer `Result` tests).

### Testing pure functions

Design business logic as pure functions that accept explicit parameters instead of reading from global singletons. This makes them testable without filesystem or process state:

```rust
// Testable: accepts explicit list
pub fn find_jdk_by_version_in_list(version: &str, jdks: &[JdkEntry]) -> Option<&JdkEntry> { ... }

// Not directly testable (reads global OnceLock)
pub fn find_jdk_by_version(version: &str) -> Option<&JdkEntry> { ... }
```

### Testing clap CLI parsing

**Never use `Cli::parse()` in tests** — it calls `process::exit` on error. Always use `try_parse_from`:

```rust
#[test]
fn test_use_command_parses_version() {
    let cli = Cli::try_parse_from(["sjvm", "use", "17"]).unwrap();
    match cli.command {
        Commands::Use { version, local } => {
            assert_eq!(version, "17");
            assert!(!local);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn test_unknown_flag_returns_error() {
    assert!(Cli::try_parse_from(["sjvm", "--bad-flag"]).is_err());
}
```

For snapshot testing of `--help` output (optional):
```rust
// Requires the `insta` dev-dependency
assert_snapshot!(Cli::command().render_help().to_string());
```

### Testing TUI widgets (ratatui)

Use `TestBackend` — it does not require a real terminal:

```rust
#[cfg(feature = "ui")]
#[test]
fn test_renders_jdk_list() -> anyhow::Result<()> {
    use ratatui::{backend::TestBackend, Terminal};

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|f| render_ui(f, &app_state))?;

    let buffer = terminal.backend().buffer().clone();
    // Assert specific cells or use snapshot testing with insta
    assert!(buffer.content().iter().any(|c| c.symbol() == "17"));
    Ok(())
}
```

Note: `TestBackend` uses `core::convert::Infallible` as its error type in ratatui 0.30+.

---

## Mocking & Fixtures

- **No mocking framework** is used in this project. Instead, design functions to accept injected parameters (paths, lists, etc.) so tests can pass test data directly.
- **Test config**: place test configuration files in `test-config/` directory.
- **Filesystem tests**: use `std::env::temp_dir()` to create temporary files/directories; clean up in the test.
- **Avoid global state in tests**: do not rely on `OnceLock` singletons in unit tests — pass data explicitly.

---

## Running Tests

### Unit tests (standard development — always use this)

```bash
# Preferred (MCP tools)
rust-mcp-server_cargo-test                        # Run all unit tests
rust-mcp-server_cargo-test --testname test_name   # Run specific test by name

# Alternative (standard Cargo)
cargo test                                        # Run all unit + doc tests
cargo test test_name                              # Run tests matching name
cargo test --no-default-features                 # Test minimal feature set
cargo test --features ui                          # Test with TUI feature enabled
cargo test --all-features                         # Main local/CI validation path
```

### E2E tests (Docker only — run only when specifically requested)

**Never run E2E tests directly** — they require Docker with Ubuntu 22.04, Java 11, 17, and 21:

```bash
# Start Docker environment and run E2E tests
docker compose -f ./docker/it-ubuntu-compose.yaml up --build

# Tear down after tests
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

Inside Docker, E2E tests are run as:
```bash
cargo test --all-features --test e2e test_setup -- --ignored
cargo test --all-features --test e2e -- --skip test_setup --ignored --test-threads=1
```

---

## Coverage Requirements

No enforced coverage threshold currently. Recommended tools:

- **Linux**: `cargo tarpaulin --out Html` — generates HTML coverage report.
- **Cross-platform**: `cargo llvm-cov` — alternative based on LLVM instrumentation.

Focus coverage efforts on:
- `jdk_switcher.rs` — pure functions with high testability.
- `config.rs` — config parsing and merging logic.
- `symlinks.rs` — platform-specific logic (test on both platforms if possible).

---

## CI Test Recommendations

CI uses stable as the main quality gate and a separate MSRV 1.88 compatibility job. Recommended commands:

```bash
cargo fmt --check              # Fail on formatting diff
cargo check --all-features     # Fail on type errors
cargo test --all-features      # Fail on test failures
cargo clippy --all-features -- -D warnings    # Fail on lint warnings
cargo +1.88 check --all-features && cargo +1.88 test --all-features
cargo audit                    # Fail on known vulnerabilities
```

# AGENTS.md

Guidelines for AI agents working on **sjvm** - a Rust CLI for managing Java JDK installations via symlinks.

## Build & Lint Commands

```bash
# Use rust-mcp-server tools when available (preferred)
rust-mcp-server_cargo-check    # Fast type checking
rust-mcp-server_cargo-clippy   # Linting  
rust-mcp-server_cargo-fmt      # Format code
rust-mcp-server_cargo-build    # Build project

# Alternative commands
cargo build                    # Debug build
cargo build --release          # Release build
```

## Testing Strategy

### Unit Tests (Standard Development)

**Always use `rust-mcp-server_cargo-test` for unit testing.**

```bash
rust-mcp-server_cargo-test                        # Run all unit tests
rust-mcp-server_cargo-test --testname test_name   # Run specific test
```

Unit tests are in `#[cfg(test)]` modules within source files (e.g., `src/ui_command.rs`).

### E2E Tests (Special Task - Separate Workflow)

**E2E tests are NOT part of standard development.** They require Docker and are worked on separately.

```bash
# Run ONLY via Docker when specifically requested
docker compose -f ./docker/it-ubuntu-compose.yaml up --build
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

**Never run e2e tests directly** - they need Docker with Java 11/17/21.

## Project Structure

```
src/
├── main.rs           # CLI entry (clap Parser/Subcommand)
├── config.rs         # JSON config with OnceLock singleton
├── jdk_resolver.rs   # JDK discovery
├── symlinks.rs       # Cross-platform symlink ops
├── memory.rs         # Binary cache (bincode)
├── app_dirs.rs       # Platform directories
├── ui_command.rs     # Interactive TUI (ratatui)
├── *_command.rs      # CLI command implementations
tests/e2e.rs          # Integration tests (Docker-only)
```

## Code Style

### Import Order
```rust
// 1. Standard library (grouped)
use std::{fs, path::{Path, PathBuf}, sync::OnceLock};

// 2. External crates
use anyhow::Context;
use clap::{Parser, Subcommand};

// 3. Local modules
use crate::config::config;
```

### Naming Conventions
- **Modules/Functions/Files**: `snake_case` (`jdk_resolver.rs`, `use_version()`)
- **Structs/Enums**: `PascalCase` (`Config`, `Commands`)
- **Statics**: `SCREAMING_SNAKE_CASE` (`static CONFIG: OnceLock`)

### Error Handling
```rust
use anyhow::Context;

// Use with_context for descriptive errors
fs::read(path).with_context(|| "Cannot read config file")?;

// Return Result<T, anyhow::Error> from fallible functions
pub fn create_symlink(target: &Path, link: &Path) -> Result<(), anyhow::Error>

// unwrap() acceptable in main command functions only
```

### Singleton Pattern
```rust
static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().unwrap())
}
```

### Platform-Specific Code
```rust
#[cfg(target_os = "windows")]
std::os::windows::fs::symlink_dir(target, link)?;

#[cfg(unix)]
std::os::unix::fs::symlink(target, link)?;

// Runtime check
if cfg!(target_os = "windows") { /* Windows */ } else { /* Unix */ }
```

### CLI Structure (Clap Derive)
```rust
#[derive(Parser)]
#[command(name = "sjvm", version = "1.0", about = "Java version manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Setup,
    Use { version: String, #[arg(short, long)] local: bool },
    List,
}
```

### User Feedback
```rust
println!("✅ Now using JDK: {}", jdk.to_string_lossy());
println!("❌ JDK version '{}' not found.", version);
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI parsing (derive) |
| `anyhow` | Error handling |
| `serde`/`serde_json` | JSON config |
| `bincode` | Binary cache |
| `directories` | Cross-platform paths |
| `ratatui`/`crossterm` | Terminal UI |

## Avoid

- Running e2e tests directly (use Docker)
- Using `panic!()` in library code (return `Result`)
- Hardcoding paths (use platform detection)
- Adding heavy dependencies without justification

## Environment

- **Rust Edition**: 2024 | **Min Version**: 1.85
- **E2E Docker**: Ubuntu 22.04 with Java 11, 17, 21

## Quick Reference

```bash
# Standard workflow
rust-mcp-server_cargo-check && rust-mcp-server_cargo-test && rust-mcp-server_cargo-clippy

# Build and run
cargo build && ./target/debug/sjvm --help
```

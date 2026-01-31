# AGENTS.md

This file provides guidelines and commands for agentic coding agents working on the sjvm (Simple Java Version Manager) project.

## Project Overview

**sjvm** is a Rust-based CLI tool for managing multiple Java JDK installations using symlink indirection. It's a minimalist, cross-platform Java version manager similar to tools like jenv or sdkman, but with a simpler symlink-based approach.

## Build, Test, and Development Commands

### Core Commands
```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run the CLI with built binary
./target/release/sjvm --help

# Run tests
cargo test

# Run a specific test
cargo test test_name
cargo test test_java_21

# Run ignored tests (for comprehensive integration testing)
cargo test -- --ignored

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy lints
cargo clippy

# Run clippy with all targets and features
cargo clippy --all-targets --all-features
```

### Integration Testing with Docker
```bash
# Run integration tests in Docker (image builds automatically)
docker compose -f ./docker/it-ubuntu-compose.yaml up

# Or run in detached mode
docker compose -f ./docker/it-ubuntu-compose.yaml up -d

# Force rebuild of the Docker image
docker compose -f ./docker/it-ubuntu-compose.yaml up --build

# Stop containers
docker compose -f ./docker/it-ubuntu-compose.yaml down

# View logs
docker compose -f ./docker/it-ubuntu-compose.yaml logs -f
```

## Project Structure and Architecture

### Core Modules
- `main.rs` - CLI entry point and command routing using clap
- `config.rs` - Configuration management with JSON persistence
- `jdk_resolver.rs` - JDK discovery and version detection
- `symlinks.rs` - Cross-platform symlink operations
- `memory.rs` - In-memory JDK management and caching
- `*_command.rs` - Individual CLI command implementations

### Module Organization
- Each CLI command has its own module file (e.g., `use_command.rs`, `list_command.rs`)
- Shared utilities are in dedicated modules (`app_dirs.rs`, `symlinks.rs`)
- Configuration is centralized in `config.rs` with singleton pattern using `OnceLock`

## Code Style Guidelines

### Imports and Dependencies
```rust
// Standard library imports first, grouped alphabetically
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

// External crates second, grouped by crate
use anyhow::Context;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

// Local modules last
use crate::config::config;
use crate::symlinks::create_symlink;
```

### Naming Conventions
- **Modules**: snake_case (e.g., `jdk_resolver.rs`, `use_command.rs`)
- **Functions**: snake_case (e.g., `use_version()`, `get_symlink_path()`)
- **Structs**: PascalCase (e.g., `Config`, `Cli`, `Commands`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `CONFIG`)
- **File names**: snake_case with underscores

### Error Handling
- Use `anyhow` for error handling with context
- Use `with_context()` for descriptive error messages
- Prefer `Result<T, anyhow::Error>` return types
- Use `unwrap()` only in main command functions where panic is acceptable
- Example:
```rust
std::fs::remove_file(link).with_context(|| "Cannot remove symlink")?;
```

### CLI Structure with Clap
- Use derive macros for CLI parsing
- Commands are structured as subcommands
- Use descriptive `about` text
- Example:
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

### Configuration Management
- Use JSON for configuration with `serde`
- Provide sensible defaults with `Default` trait
- Use `OnceLock` for singleton configuration pattern
- Cross-platform config directories using `directories` crate
- Configuration merge strategy: defaults + user overrides

### Platform-Specific Code
- Use conditional compilation for platform differences
```rust
#[cfg(target_os = "windows")]
// Windows-specific code

#[cfg(unix)]
// Unix-specific code

if cfg!(target_os = "windows") {
    // Runtime platform check
}
```

### Testing Guidelines
- Integration tests in `tests/` directory
- Use `Command::new()` for CLI testing
- Test against real `./target/release/sjvm` binary
- Use `#[ignore]` for comprehensive integration tests that require Docker
- Assert on both success status and output content
- Example:
```rust
#[test]
fn test_cli_runs_successfully() {
    let output = Command::new("./target/release/sjvm")
        .arg("--version")
        .output()
        .expect("failed to execute process");
    
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sjvm"));
}
```

## Dependencies and Their Usage

### Core Dependencies
- `clap` v4.5 - CLI parsing with derive features
- `anyhow` v1.0 - Error handling with context
- `serde` v1.0 - JSON serialization/deserialization
- `directories` v6 - Cross-platform config directories
- `walkdir` v2.5 - Directory traversal for JDK discovery

### When Adding Dependencies
1. Check if functionality can be implemented with std library first
2. Prefer minimal, well-maintained crates
3. Update `Cargo.toml` with exact versions when possible
4. Consider cross-platform compatibility

## Code Patterns to Follow

### Singleton Pattern for Configuration
```rust
static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().unwrap())
}
```

### Path Operations
- Use `PathBuf` for owned paths
- Use `to_string_lossy()` for display
- Use `with_context()` for file operation errors

### User Output
- Use emojis for visual feedback (✅ ❌)
- Provide clear error messages
- Show commands to run on Windows when local mode isn't supported

## Things to Avoid

- Don't commit `.gitignore` changes unless necessary
- Don't add complex external dependencies without discussion
- Don't use `panic!()` in library code - prefer `Result`
- Don't hardcode platform-specific paths without feature gates
- Don't ignore cross-platform compatibility

## Development Environment

- Rust edition 2024
- Minimum Rust version: 1.85 (edition 2024)
- Test environment: Docker Ubuntu 22.04 with Java 11, 17, 21
- Integration testing requires Docker setup

## File Organization

- Keep CLI commands in separate modules
- One public struct/function per module when possible
- Use module-level documentation (///) for public APIs
- Keep integration tests separate from unit tests

This project follows a minimalist philosophy while maintaining robust cross-platform support and comprehensive testing.

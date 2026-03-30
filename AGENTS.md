# AGENTS.md

Guidelines for AI agents working on **sjvm** - a Rust CLI for managing Java JDK installations via symlinks.

Detailed guidelines have been migrated to `.project-guidelines-for-ai/`. Read those files before writing any code.

- **Coding**: [`.project-guidelines-for-ai/coding/coding-guidelines.md`](.project-guidelines-for-ai/coding/coding-guidelines.md)
- **Code examples**: [`.project-guidelines-for-ai/coding/code-examples/README.md`](.project-guidelines-for-ai/coding/code-examples/README.md)
- **Building**: [`.project-guidelines-for-ai/building/building-guidelines.md`](.project-guidelines-for-ai/building/building-guidelines.md)
- **Testing**: [`.project-guidelines-for-ai/testing/testing-guidelines.md`](.project-guidelines-for-ai/testing/testing-guidelines.md)
- **Documentation**: [`.project-guidelines-for-ai/documentation/documentation-guidelines.md`](.project-guidelines-for-ai/documentation/documentation-guidelines.md)
- **Security**: [`.project-guidelines-for-ai/security/security-guidelines.md`](.project-guidelines-for-ai/security/security-guidelines.md)

---

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

Unit tests are in `#[cfg(test)]` modules within source files (e.g., `src/commands/ui/install_screen.rs`).

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
├── core/
│   ├── mod.rs
│   ├── jdk_catalog.rs    # Vendor API integration (Adoptium, GraalVM CE)
│   ├── downloader.rs     # Streaming download, SHA-256 verify, extract, install
│   ├── jdk_resolver.rs   # JDK discovery
│   └── jdk_switcher.rs   # JDK version lookup and symlink switching
├── infra/
│   ├── mod.rs
│   ├── app_dirs.rs       # Platform directories
│   ├── config.rs         # JSON config with OnceLock singleton
│   ├── http.rs           # reqwest blocking client (rustls-tls)
│   ├── memory.rs         # Binary cache (bincode)
│   └── symlinks.rs       # Cross-platform symlink ops
└── commands/
    ├── mod.rs
    ├── delete.rs         # Removes an installed JDK (with confirmation)
    ├── install.rs        # Downloads and installs a JDK
    ├── list.rs           # Lists known JDKs
    ├── setup.rs          # First-run setup
    ├── versions.rs       # Lists available versions from vendor APIs
    ├── ui/               # Interactive TUI (ratatui, feature-gated)
    │   ├── mod.rs        # Screen enum, tab bar, event loop
    │   ├── switch_screen.rs  # JDK switcher screen
    │   └── install_screen.rs # JDK install screen with inline progress
    └── use_cmd.rs        # Switches JDK globally or locally
tests/e2e.rs              # Integration tests (Docker-only)
```

## Environment

- **Rust Edition**: 2024 | **Min Version**: 1.88 | **Local Default Toolchain**: stable (`rust-toolchain.toml`)
- **E2E Docker**: Ubuntu 22.04 with Java 11, 17, 21

## Quick Reference

```bash
# Standard workflow
rust-mcp-server_cargo-check --all-features && rust-mcp-server_cargo-test --all-features && rust-mcp-server_cargo-clippy --all-features -- -D warnings

# Build and run
cargo build && ./target/debug/sjvm --help
```

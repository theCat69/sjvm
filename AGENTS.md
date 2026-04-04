# AGENTS.md

Guidelines for AI agents working on **sjvm** — a Rust CLI for managing Java JDK installations via symlinks.

## Skill-Based Guidelines (primary reference)

Detailed, loadable skill files live in `.opencode/skills/`. Load the relevant skill before writing code:

| Skill | File | Covers |
|-------|------|--------|
| **Coding** | [`.opencode/skills/project-coding/SKILL.md`](.opencode/skills/project-coding/SKILL.md) | Code style, naming, error handling, architecture patterns |
| **Build** | [`.opencode/skills/project-build/SKILL.md`](.opencode/skills/project-build/SKILL.md) | Build commands, CI/CD, Cargo.lock policy, feature flags |
| **Test** | [`.opencode/skills/project-test/SKILL.md`](.opencode/skills/project-test/SKILL.md) | Test location, framework, patterns, running tests |
| **Documentation** | [`.opencode/skills/project-documentation/SKILL.md`](.opencode/skills/project-documentation/SKILL.md) | Rustdoc, README, changelog standards |
| **Security** | [`.opencode/skills/project-security/SKILL.md`](.opencode/skills/project-security/SKILL.md) | Secrets, input validation, dependency security, CVEs |
| **Code Examples** | [`.opencode/skills/project-code-examples/SKILL.md`](.opencode/skills/project-code-examples/SKILL.md) | Index of pattern examples in `.code-examples-for-ai/` |

Code examples (annotated snippets from production code) are in [`.code-examples-for-ai/`](.code-examples-for-ai/).

Legacy guideline files remain in `.project-guidelines-for-ai/` for reference.

---

## Quick Reference

```bash
# Standard workflow (run before every commit)
rust-mcp-server_cargo-check --all-features && rust-mcp-server_cargo-test --all-features && rust-mcp-server_cargo-clippy --all-features -- -D warnings

# Build and run
cargo build && ./target/debug/sjvm --help
```

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

Unit tests are in `#[cfg(test)]` modules within source files.

### E2E Tests (Special Task — Separate Workflow)

**E2E tests are NOT part of standard development.** They require Docker and are worked on separately.

```bash
# Run ONLY via Docker when specifically requested
docker compose -f ./docker/it-ubuntu-compose.yaml up --build
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

**Never run e2e tests directly** — they need Docker with Java 11/17/21.

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

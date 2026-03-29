# Building Guidelines

Build instructions and conventions for **sjvm** — a Rust 2024 edition CLI binary.

---

## Prerequisites

- **Rust**: minimum version 1.86 (edition 2024). Install via [rustup](https://rustup.rs/).
  ```bash
  rustup install stable
  rustup update
  rustc --version   # must be >= 1.86
  ```
- **Cargo**: bundled with Rust; no separate installation needed.
- **Docker** (E2E tests only): required for integration tests; not needed for standard builds or unit tests.
- **`rust-mcp-server` MCP tools** (preferred in AI agent context): available as `rust-mcp-server_cargo-*` commands.

---

## Environment Setup

1. Clone the repository.
2. Verify Rust version: `rustc --version` — must be ≥ 1.86.
3. No additional environment variables or system dependencies are required for a debug build.
4. For E2E tests, Docker must be installed and running (see Testing Guidelines).

---

## Build Commands

### Preferred (MCP tools — use these in AI agent context)

```bash
rust-mcp-server_cargo-check    # Fast type checking (no codegen)
rust-mcp-server_cargo-clippy   # Linting — fix all warnings before committing
rust-mcp-server_cargo-fmt      # Format all code
rust-mcp-server_cargo-build    # Debug build
```

### Standard Cargo Commands (alternative)

```bash
cargo check                    # Fast type checking
cargo clippy -- -D warnings    # Lint — fail on any warning
cargo fmt                      # Format code
cargo fmt --check              # Verify formatting without changing files (CI)
cargo build                    # Debug build → target/debug/sjvm
cargo build --release          # Release build → target/release/sjvm
```

### Feature Flags

The `ui` feature enables the optional TUI (ratatui + crossterm):

```bash
cargo build --features ui              # Build with TUI support
cargo build --release --features ui    # Release build with TUI
cargo build --no-default-features      # Minimal build (no TUI)
```

Default features are empty (`default = []`), so the base build has no TUI.

---

## Development Workflow

Standard AI agent workflow — run these in order before every commit:

```bash
rust-mcp-server_cargo-check && rust-mcp-server_cargo-test && rust-mcp-server_cargo-clippy
```

Or with standard Cargo:

```bash
cargo fmt --check && cargo check && cargo test && cargo clippy -- -D warnings
```

Run the binary after a debug build:

```bash
cargo build && ./target/debug/sjvm --help
```

---

## Release Profile Configuration

The `Cargo.toml` release profile should be configured for optimized binaries. Recommended settings:

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

- `lto = true`: enables link-time optimization for smaller, faster binaries.
- `codegen-units = 1`: slower compile, better optimization.
- `strip = true`: removes debug symbols from the binary (reduces size).

---

## Cargo.lock Policy

**Cargo.lock must be committed** for this binary crate. This ensures:
- Reproducible builds across machines.
- CI and Docker builds use the exact same dependency versions.
- Security: pinned versions prevent unexpected upgrades from introducing vulnerabilities.

Do **not** add `Cargo.lock` to `.gitignore` (it is correct for library crates, but wrong for binaries).

---

## Feature Flags

| Feature | Enables | Default |
|---------|---------|---------|
| `ui` | `ratatui` + `crossterm` (TUI interactive mode) | ❌ off |

- All TUI code must be gated with `#[cfg(feature = "ui")]`.
- The `ui` feature adds `ratatui 0.30.0` and `crossterm 0.29.0` as optional dependencies.
- **Compatibility note**: ratatui 0.30.0 declares MSRV 1.86, which matches the project's MSRV 1.86.

---

## CI/CD Pipeline

There is currently no GitHub Actions pipeline. When one is added, the recommended CI jobs are:

```yaml
# Recommended CI steps (GitHub Actions example)
- name: Format check
  run: cargo fmt --check

- name: Type check
  run: cargo check --all-features

- name: Lint
  run: cargo clippy --all-features -- -D warnings

- name: Unit tests
  run: cargo test

- name: Security audit
  run: cargo audit

- name: Release build
  run: cargo build --release
```

E2E tests run separately via Docker:

```bash
docker compose -f ./docker/it-ubuntu-compose.yaml up --build
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

**Never run E2E tests directly** (outside Docker) — they require Ubuntu 22.04 with Java 11, 17, and 21 installed.

---

## Dependency Management

- Prefer caret ranges (`"4.5"`) over exact pins (`"= 4.5.0"`) in `Cargo.toml`.
- Disable default features when only specific features are needed: `default-features = false`.
- Do not add heavy dependencies without justification — evaluate alternatives first.
- Run `cargo outdated` periodically to detect stale dependencies.
- Run `cargo audit` to check against the RustSec advisory database.

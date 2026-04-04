---
name: project-build
description: Project-specific build commands, prerequisites, environment setup, and CI/CD pipeline
---

# Project Build Guidelines — sjvm

**sjvm** is a Rust 2024 edition CLI binary. Build system: Cargo. MSRV: 1.88.

---

## Prerequisites

- **Rust**: minimum version 1.88 (edition 2024). Install via [rustup](https://rustup.rs/).
  ```bash
  rustup install stable
  rustup update
  rustc --version   # must be >= 1.88
  ```
- **Cargo**: bundled with Rust; no separate installation required.
- **Docker** (E2E tests only): required for integration tests; not needed for standard builds.
- **`rust-mcp-server` MCP tools** (preferred in AI agent context): available as `rust-mcp-server_cargo-*` commands.

---

## Environment Setup

1. Clone the repository.
2. Verify Rust version: `rustc --version` — must be ≥ 1.88.
3. No additional environment variables or system dependencies are required for a debug build.
4. For E2E tests, Docker must be installed and running (see CI/CD section).
5. Optional: set `GITHUB_TOKEN` environment variable to increase GitHub API rate limits during GraalVM version resolution.

---

## Build Commands

### Preferred (MCP tools — use in AI agent context)

```bash
rust-mcp-server_cargo-check    # Fast type checking (use --all-features when relevant)
rust-mcp-server_cargo-clippy   # Linting — use all-features for the main quality gate
rust-mcp-server_cargo-fmt      # Format all code
rust-mcp-server_cargo-build    # Debug build
```

### Standard Cargo Commands (alternative)

```bash
cargo check --all-features              # Fast type checking for all feature-gated code
cargo clippy --all-features -- -D warnings   # Lint — fail on any warning
cargo fmt                               # Format code
cargo fmt --check                       # Verify formatting without changing files (CI)
cargo build                             # Debug build → target/debug/sjvm
cargo build --release                   # Release build → target/release/sjvm
```

### Feature Flags

The `ui` feature enables the optional ratatui TUI:

```bash
cargo build --features ui              # Build with TUI support
cargo build --release --features ui    # Release build with TUI
cargo build --no-default-features      # Minimal build (no TUI)
```

Default features are empty (`default = []`), so the base build has no TUI. All TUI code must be gated with `#[cfg(feature = "ui")]`.

---

## Development Workflow

Standard AI agent workflow — run these in order before every commit:

```bash
# Preferred (MCP tools)
rust-mcp-server_cargo-check --all-features && rust-mcp-server_cargo-test --all-features && rust-mcp-server_cargo-clippy --all-features -- -D warnings

# Alternative (standard Cargo)
cargo fmt --check && cargo check --all-features && cargo test --all-features && cargo clippy --all-features -- -D warnings
```

Run the binary after a debug build:

```bash
cargo build && ./target/debug/sjvm --help
```

---

## Release Profile Configuration

Current `Cargo.toml` release profile (optimized binary):

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

- `lto = true`: link-time optimization — smaller, faster binary.
- `codegen-units = 1`: slower compile, better optimization.
- `strip = true`: removes debug symbols (reduces binary size).

---

## Cargo.lock Policy

**`Cargo.lock` must be committed** for this binary crate. This ensures:
- Reproducible builds across machines and CI.
- Security: exact pinned versions prevent unexpected upgrades.
- `cargo audit` operates on the committed lock file.

Never add `Cargo.lock` to `.gitignore` (correct for library crates, wrong for binaries).

---

## Security-Pinned Dependencies

Some dependencies are pinned to exact versions in `Cargo.toml` due to CVEs:

```toml
tar = "=0.4.45"   # RUSTSEC-2026-0067/0068 — path traversal in tar extraction
zip = "=2.3.0"    # CVE-2025-29787 — path traversal in zip extraction
time = "0.3.47"   # RUSTSEC-2026-0009 / CVE-2026-25727 — security pin
```

When updating these, verify the new version resolves the advisory and update the comment.

---

## CI/CD Pipeline

GitHub Actions CI (`.github/workflows/ci.yml`) — three jobs:

### `ci` job (stable toolchain — main quality gate)

```bash
cargo fmt --check                              # Fail on formatting diff
cargo check --all-features                     # Fail on type errors
cargo clippy --all-features -- -D warnings     # Fail on lint warnings
cargo test --all-features                      # Fail on test failures
cargo build --release                          # Verify release build compiles
```

### `msrv` job (Rust 1.88 — MSRV compatibility)

```bash
cargo +1.88 check --all-features
cargo +1.88 test --all-features
```

### `audit` job (security)

```bash
cargo audit    # Check against RustSec Advisory Database — fail on any known vulnerability
```

### E2E tests (Docker — run only when specifically requested)

```bash
docker compose -f ./docker/it-ubuntu-compose.yaml up --build
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

**Never run E2E tests directly** — they require Ubuntu 22.04 with Java 11, 17, and 21 installed.

---

## Dependency Management

- Use caret ranges (`"4.5"`) for most deps; `Cargo.lock` pins the exact version for builds.
- Use `=x.y.z` exact pins **only** for security-sensitive deps with known CVE history.
- `default-features = false` whenever only specific features are needed — reduces attack surface and compile time.
- Do not add heavy dependencies without justification.
- Run `cargo outdated` periodically to detect stale dependencies.
- Run `cargo audit` on every `Cargo.lock` change.

---

## Feature Flags

| Feature | Enables | Default |
|---------|---------|---------|
| `ui` | `ratatui 0.30.0` + `crossterm 0.29.0` (interactive TUI) | ❌ off |

- ratatui 0.30.0 is compatible with MSRV 1.88.
- Verify compatibility before upgrading ratatui.

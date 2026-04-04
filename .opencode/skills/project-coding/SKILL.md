---
name: project-coding
description: Project-specific coding guidelines, naming conventions, architecture patterns, and code examples
---

# Project Coding Guidelines — sjvm

**sjvm** is a Rust 2024 edition CLI (MSRV 1.88) for managing Java JDK installations via symlinks.

---

## Code Style

- **Indentation**: 4 spaces (rustfmt default). Never use tabs.
- **Max line length**: 100 characters (rustfmt default).
- **Formatting**: All code must pass `cargo fmt` without changes. Format before every commit.
- **Linting**: All code must pass `cargo clippy --all-features -- -D warnings`. Fix warnings; suppress only with explicit justification and a comment explaining why.
- **Edition**: Rust 2024. Use edition-2024 idioms:
  - `if let` chains (`if let A = x && let B = y {}`) — stable in 2024; prefer over nested `if let`.
  - `unsafe_op_in_unsafe_fn` is deny-by-default in 2024 — every `unsafe` call inside an `unsafe fn` still requires its own `unsafe` block.
  - Temporary lifetimes in match arms are shorter in 2024 — do not rely on extended temporary lifetime.
- **User-facing output**: Use emoji prefixes in all `println!` / `eprintln!` output:
  - `✅` for success
  - `❌` for errors
  - `→` to mark the current/active item (e.g. active JDK in `sjvm list`)

---

## Naming Conventions

Follow [Rust API Guidelines (RFC 430)](https://rust-lang.github.io/api-guidelines/naming.html):

| Item | Convention | Example |
|------|-----------|---------|
| Modules, functions, methods, local variables, file names | `snake_case` | `jdk_resolver.rs`, `use_version()` |
| Structs, Enums, Traits, Type aliases | `UpperCamelCase` | `Config`, `Commands`, `ArtifactInfo` |
| Enum variants | `UpperCamelCase` | `Commands::Setup`, `Vendor::GraalVm` |
| Constants and statics | `SCREAMING_SNAKE_CASE` | `static CONFIG: OnceLock<Config>` |
| Type parameters | Concise `UpperCamelCase`, usually single letter | `T`, `E` |
| Lifetimes | Short lowercase, usually single letter | `'a` |
| Acronyms in `UpperCamelCase` | Treat as one word | `Uuid` not `UUID`, `Stdin` not `StdIn` |

**Conversion method prefixes** (Rust API Guidelines):
- `as_` — free, borrowed → borrowed (e.g. `as_str()`)
- `to_` — potentially expensive, borrowed → owned (e.g. `to_string()`)
- `into_` — consumes self, owned → owned (e.g. `into_iter()`)

**Getter convention**: No `get_` prefix. Use bare name: `fn first(&self) -> &First`, not `fn get_first()`.

**Feature names**: direct noun form — `ui`, not `with-ui` or `use-ui`.

---

## Import Ordering

Imports must be grouped in this order with a blank line between groups. `rustfmt` enforces this automatically:

```rust
// 1. Standard library — use grouped form
use std::{fs, path::{Path, PathBuf}, sync::OnceLock};

// 2. External crates — one use per crate or logically grouped
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

// 3. Local modules — crate:: prefix
use crate::infra::config::config;
```

- Never mix std and external crates in the same `use` group.
- Prefer grouped `use std::{...}` over multiple `use std::...` lines.

---

## Error Handling

- **Application code uses `anyhow`**. Return `anyhow::Result<T>` from all fallible functions.
- **Never use `thiserror`** for command implementations — anyhow is the correct choice for this binary.
- **Standard import**:
  ```rust
  use anyhow::{bail, Context, Result};
  ```
- **Always attach context** at every error boundary — never bare `?`:
  ```rust
  // Static context (no allocation when Ok)
  fs::read_to_string(path).context("Failed to read config file")?;

  // Dynamic context (lazy; preferred when formatting is needed)
  fs::read_to_string(path)
      .with_context(|| format!("Failed to read config from {}", path.display()))?;
  ```
- **Option → Result**:
  ```rust
  value.context("Expected a value but got None")?;
  ```
- **Inline error construction**:
  ```rust
  bail!("unexpected version: {version}");          // early return Err
  ensure!(x > 0, "x must be positive, got {x}");  // assert that returns Err
  ```
- **Context string convention**: `"Failed to <action> <subject>"` — safe to display to end users; never include secrets, tokens, or private paths.
- **`unwrap()` policy**:
  - Acceptable **only** in `main` command dispatch (`if let Err(e) = ... { eprintln!(...); exit(1); }`) and `#[cfg(test)]` test bodies.
  - **Never** use `.unwrap()` in library logic, helpers, or any path reachable from tests.
  - Replace with `?` + `.context("reason")`.

---

## Patterns & Architecture

### Three-Layer Module Structure

```
src/
├── main.rs           # CLI entry — clap dispatch only; no business logic
├── core/             # Business logic (pure functions preferred)
│   ├── jdk_catalog.rs    # Vendor API types + pure parse helpers + HTTP resolvers
│   ├── downloader.rs     # Download → verify → extract → install pipeline
│   ├── jdk_resolver.rs   # JDK filesystem discovery
│   └── jdk_switcher.rs   # JDK lookup + symlink switching; pure testable functions
├── infra/            # Platform / I/O concerns
│   ├── app_dirs.rs       # Platform paths via `directories` crate
│   ├── config.rs         # JSON config with OnceLock singleton
│   ├── http.rs           # reqwest blocking client (rustls-tls only)
│   ├── memory.rs         # Binary cache (bincode 2.0)
│   └── symlinks.rs       # Cross-platform symlink operations
└── commands/         # CLI subcommand handlers
    ├── mod.rs            # shared validate_version_string()
    ├── *.rs              # one file per subcommand
    └── ui/               # ratatui TUI (feature-gated)
```

### Pure Function / HTTP Split Pattern

Separate pure parsing functions from HTTP-calling wrappers. This makes all parsing logic unit-testable without network access or mocking:

```rust
// Pure — testable with json!() fixtures, no HTTP
pub(crate) fn parse_adoptium_response(json: &Value, version: u16) -> Result<ArtifactInfo> { ... }

// HTTP wrapper — delegates to the pure function; only called in E2E / integration
fn resolve_adoptium(version: u16, os: &str, arch: &str) -> Result<ArtifactInfo> {
    let json = crate::infra::http::get_json(&url)
        .with_context(|| format!("Failed to fetch Adoptium API for JDK {version}"))?;
    parse_adoptium_response(&json, version)
}
```

### Singleton Pattern (`OnceLock` / `LazyLock`)

Use `OnceLock<T>` for lazy-initialized immutable singletons (config, dirs, HTTP clients):

```rust
static CONFIG: OnceLock<Config> = OnceLock::new();

pub(crate) fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().expect("Failed to load configuration"))
}
```

Use `LazyLock<Mutex<Option<T>>>` when the cache must be invalidatable:

```rust
static MEMORY: LazyLock<Mutex<Option<Memory>>> = LazyLock::new(|| Mutex::new(None));
```

### CLI Structure (Clap Derive API — always)

```rust
/// Java version manager via symlinks
#[derive(Parser)]
#[command(name = "sjvm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Switch the active JDK
    Use {
        /// Version string to match (e.g. "17", "temurin-21")
        #[arg(value_name = "VERSION", value_parser = validate_version)]
        version: String,
    },
}
```

- `#[command(version)]` reads version from `Cargo.toml` — never hardcode.
- `/// doc comment` on every field/variant becomes the `--help` text.
- `value_name = "VERSION"` sets uppercase metavar in help output.
- **Never use `Cli::parse()` in tests** — use `Cli::try_parse_from([...])`.

### Platform-Specific Code

Prefer compile-time `#[cfg(...)]` over runtime `cfg!()` for dead-code elimination:

```rust
#[cfg(unix)]
std::os::unix::fs::symlink(target, link)?;

#[cfg(target_os = "windows")]
std::os::windows::fs::symlink_dir(target, link)?;
```

### TUI Code (ratatui) — `ui` Feature Only

- All TUI code gated with `#[cfg(feature = "ui")]`.
- Use `ratatui::run()` (v0.30+) — handles terminal init/restore automatically.
- Implement `Widget for &Foo` (not `WidgetRef for Foo`) — preferred in ratatui 0.30.
- Background work: use `std::thread::spawn` + `mpsc::channel` to keep the event loop responsive.

### Visibility

- `pub(crate)` — expose within the crate without a public API.
- `pub(super)` — expose to parent module only.
- Avoid bare `pub` on items that don't cross crate boundaries.

### Constraints (Hard Rules)

- `#![deny(unsafe_code)]` at crate root — no unsafe blocks.
- Never `panic!()`, `unreachable!()`, or `todo!()` in production paths.
- Never hardcode paths — use `directories::ProjectDirs` via `infra/app_dirs.rs`.
- TLS must use rustls (`use_rustls_tls()`) and `danger_accept_invalid_certs(false)`.
- `GITHUB_TOKEN` must never appear in error messages or logs — use `set_sensitive(true)`.
- All new dependencies: `default-features = false` where possible.
- `Cargo.lock` must be committed (binary crate).

---

## Code Examples

See `.code-examples-for-ai/` for annotated examples of each pattern:

- `clap-subcommand.md` — Parser/Subcommand derive, try_parse_from in tests
- `anyhow-error-handling.md` — with_context, bail!, ensure!, ? propagation
- `oncelock-singleton.md` — OnceLock and LazyLock<Mutex<Option<T>>> patterns
- `pure-parse-http-split.md` — separating parse logic from HTTP calls for testability
- `config-oncelock-singleton.md` — Config singleton with partial JSON merge and path validation
- `bincode-cache.md` — bincode 2.0 Encode/Decode derive and round-trip tests
- `unit-test-patterns.md` — #[cfg(test)], use super::*, fixture patterns
- `platform-symlinks.md` — #[cfg(target_os)] symlink guards and cross-device rename
- `https-only-http-client.md` — require_https guard, sensitive headers, OnceLock client

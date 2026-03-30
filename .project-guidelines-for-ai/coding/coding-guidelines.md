# Coding Guidelines

This document defines the coding conventions for **sjvm** — a Rust 2024 edition CLI for managing Java JDK installations via symlinks. All guidelines apply to Rust 2024 edition, MSRV 1.88.

---

## Code Style

- **Indentation**: 4 spaces (rustfmt default). Never use tabs.
- **Max line length**: 100 characters (rustfmt default).
- **Formatting**: All code must pass `cargo fmt` / `rust-mcp-server_cargo-fmt` without changes. Format before committing.
- **Linting**: All code must pass `cargo clippy --all-features -- -D warnings` as the main validation path. CI may also run a no-features pass; fix clippy warnings and do not suppress them without justification.
- **Edition**: Rust 2024. Use edition-2024 idioms:
  - `if let` chains are expressions — use them where cleaner than nested `if let`.
  - `unsafe_op_in_unsafe_fn` lint is deny-by-default in 2024 — every `unsafe` call inside an `unsafe fn` still requires its own `unsafe` block.
  - Temporary lifetimes in match arms are shorter in 2024 — do not rely on extended temporary lifetime.
- **User feedback**: Use emoji prefixes in all user-facing `println!` output:
  - `✅` for success
  - `❌` for errors
  - `→` to mark the current/active item (e.g. current JDK in list)

---

## Naming Conventions

Follow [Rust API Guidelines (RFC 430)](https://rust-lang.github.io/api-guidelines/naming.html):

| Item | Convention | Example |
|------|-----------|---------|
| Modules, functions, methods, local variables, file names | `snake_case` | `jdk_resolver.rs`, `use_version()` |
| Structs, Enums, Traits, Type aliases | `UpperCamelCase` | `Config`, `Commands`, `JdkEntry` |
| Enum variants | `UpperCamelCase` | `Commands::Setup`, `Commands::Use` |
| Constants and statics | `SCREAMING_SNAKE_CASE` | `static CONFIG: OnceLock<Config>` |
| Type parameters | Concise `UpperCamelCase`, usually single letter | `T`, `E` |
| Lifetimes | Short lowercase, usually single letter | `'a` |
| Macros | `snake_case!` | `bail!`, `ensure!` |
| Acronyms in `UpperCamelCase` | Treat as one word | `Uuid` not `UUID`, `Stdin` not `StdIn` |

**Conversion method prefixes** (Rust API Guidelines):
- `as_` — free, borrowed → borrowed (e.g. `as_str()`)
- `to_` — potentially expensive, borrowed → owned (e.g. `to_string()`)
- `into_` — consumes self, owned → owned (e.g. `into_iter()`)

**Getter convention**: No `get_` prefix. Use bare name: `fn first(&self) -> &First`, not `fn get_first()`.

**Do NOT** suffix/prefix crate names with `-rs` or `-rust`.

---

## Import Ordering

Imports must be grouped in this order, separated by a blank line between each group. `rustfmt` enforces this automatically:

```rust
// 1. Standard library — use grouped form
use std::{fs, path::{Path, PathBuf}, sync::OnceLock};

// 2. External crates — one use per crate or logically grouped
use anyhow::Context;
use clap::{Parser, Subcommand};

// 3. Local modules — crate:: prefix
use crate::config::config;
```

- Never mix std and external crates in the same `use` group.
- Prefer grouped `use std::{...}` over multiple `use std::...` lines.

---

## Error Handling

- **Application code uses `anyhow`**. Return `anyhow::Result<T>` (alias for `Result<T, anyhow::Error>`) from all fallible functions.
- **Never use `thiserror` for command implementations** — anyhow is correct for this binary.
- **Standard imports**:
  ```rust
  use anyhow::{bail, Context, Result};
  ```
- **Context on errors** — always attach descriptive context at error boundaries:
  ```rust
  // Static context (no allocation)
  fs::read_to_string(path).context("Failed to read config file")?;

  // Dynamic context (use when path/value adds meaning)
  fs::read_to_string(path)
      .with_context(|| format!("Failed to read config from {}", path.display()))?;
  ```
- **Option to Result**:
  ```rust
  value.context("Expected a value but got None")?;
  ```
- **Inline error construction**:
  ```rust
  bail!("unexpected version: {version}");   // returns Err(anyhow!(...))
  ensure!(x > 0, "x must be positive, got {x}");  // assert that returns Err
  ```
- **`unwrap()` policy**:
  - Acceptable **only** in `main` command dispatch functions where panic equals program abort anyway.
  - Acceptable in `#[cfg(test)]` test bodies.
  - **Never** use `.unwrap()` in library logic, helper functions, or any path called from tests or other modules.
  - Replace `.unwrap()` with `?` + `.context("reason")` everywhere else.
- **Do NOT attach secrets** (tokens, passwords, API keys) as context strings — they appear in logs and error messages.
- **User-facing errors**: Use `Display` (`{err}`) for user messages; use `Debug` (`{err:?}`) for developer/log output showing the full chain.

---

## Patterns & Architecture

### Module Architecture

Module structure under `src/` is organized into three subfolders — `core/`, `infra/`, and `commands/`:

| File | Role |
|------|------|
| `main.rs` | CLI entry — clap `Parser`/`Subcommand` dispatch only |
| `infra/config.rs` | JSON config with `OnceLock` singleton |
| `core/jdk_resolver.rs` | JDK discovery — scans `jdks_dirs`; `OnceLock` cached |
| `core/jdk_switcher.rs` | JDK lookup and symlink switching; pure testable functions |
| `infra/symlinks.rs` | Cross-platform symlink creation/removal |
| `infra/memory.rs` | Binary cache (`bincode`) storing current JDK; persisted to `data_dir` |
| `infra/app_dirs.rs` | Platform-specific path resolution via `directories::ProjectDirs` |
| `commands/list.rs` | Lists known JDKs; marks current with `→` |
| `commands/setup.rs` | First-run setup: creates symlink, resets memory cache |
| `commands/use_cmd.rs` | Switches JDK globally or prints shell env for local use |
| `commands/ui.rs` | Optional TUI (`ratatui`) gated behind `ui` feature flag |

### Singleton Pattern

Use `OnceLock<T>` for all lazy-initialized global state (config, memory, jdks, dirs):

```rust
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().unwrap())
}
```

### Platform-Specific Code

Prefer compile-time `#[cfg(...)]` guards over runtime `cfg!()` checks:

```rust
#[cfg(target_os = "windows")]
std::os::windows::fs::symlink_dir(target, link)?;

#[cfg(unix)]
std::os::unix::fs::symlink(target, link)?;

// Runtime check (only when needed for logic, not for dead-code elimination)
if cfg!(target_os = "windows") { /* Windows */ } else { /* Unix */ }
```

### CLI Structure (Clap Derive)

Use the derive API exclusively. Keep `Cli` struct thin — dispatch immediately to business logic:

```rust
use clap::{Parser, Subcommand};

/// Java version manager via symlinks
#[derive(Parser)]
#[command(name = "sjvm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// First-run setup
    Setup,
    /// Switch active JDK
    Use {
        version: String,
        #[arg(short, long)]
        local: bool,
    },
    /// List available JDKs
    List,
}
```

- `#[command(version)]` reads version from `Cargo.toml` automatically — do not hardcode it.
- Use `/// doc comment` on structs, enums, and fields for clap help text.
- Use `value_name = "VERSION"` on `#[arg]` for uppercase metavar in help output.
- **Never use `Cli::parse()` in tests** — use `Cli::try_parse_from([...])` which returns `Result` instead of calling `process::exit`.

### TUI (ratatui) — `ui` Feature Only

- The `ratatui` and `crossterm` dependencies are **optional**, gated behind the `ui` feature flag.
- Gate all TUI code: `#[cfg(feature = "ui")]`.
- Always restore the terminal on exit — even on panic:
  ```rust
  // Use ratatui::run() which handles init/restore automatically (v0.30+)
  ratatui::run(|terminal| { ... })
  ```
- Implement `Widget for &Foo` (not `WidgetRef for Foo`) — this is the preferred pattern in ratatui 0.30.
- Note: ratatui 0.30.0 supports the project's MSRV 1.88; verify compatibility before upgrading ratatui.

### Visibility

- Use `pub(crate)` to expose items within the crate without making them part of a public API.
- Use `pub(super)` to expose to the parent module only.
- Avoid `pub` on items that do not need to be accessible from outside their module.

### No `panic!()` in Logic

- Never call `panic!()`, `unreachable!()`, or `todo!()` in production code paths.
- Return `Result<T, anyhow::Error>` from every fallible function.
- `unwrap()` is allowed only in `main` dispatch and `#[cfg(test)]` as noted above.

### No Hardcoded Paths

- Never hardcode paths. Use `directories::ProjectDirs` (via `infra/app_dirs.rs`) for platform-specific data and config directories.
- Platform defaults are defined in config and resolved at runtime.

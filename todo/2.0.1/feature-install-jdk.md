# Feature: `sjvm install` + TUI Install Screen

**Document version:** 1.0  
**Target release:** 2.0.1  
**Status:** Planning — ready for implementation  
**Date:** 2026-03-30  

---

## Table of Contents

1. [Overview](#1-overview)
2. [CLI Specification](#2-cli-specification)
3. [TUI Specification](#3-tui-specification)
4. [Architecture](#4-architecture)
5. [New Cargo Dependencies](#5-new-cargo-dependencies)
6. [API Integration](#6-api-integration)
7. [Download and Install Flow](#7-download-and-install-flow)
8. [Security Considerations](#8-security-considerations)
9. [Error Handling Catalogue](#9-error-handling-catalogue)
10. [Testing Plan](#10-testing-plan)
11. [Implementation Tasks](#11-implementation-tasks)

---

## 1. Overview

### What the feature does

`sjvm install` adds the ability to download, verify, and install a Java JDK directly from the internet without requiring the user to manage archive downloads, checksum verification, or directory layout manually. The user specifies a major version number and an optional vendor; sjvm resolves the correct artifact URL, downloads it to a temporary location, verifies the SHA-256 checksum, extracts the archive, and moves the JDK directory into `jdks_dirs[0]`. After installation the memory cache is invalidated so subsequent commands discover the new JDK immediately.

The existing interactive TUI (`sjvm ui`) gains a second screen — an **Install Screen** — where the user can pick a vendor and version from a list rendered in the terminal and observe download progress inline, without leaving the TUI.

### Why it is being added

Currently sjvm acts only as a switcher: it assumes JDKs are already installed on the system. Users must step outside sjvm to download and install JDKs, which breaks the workflow. Adding first-class install support makes sjvm a complete version manager comparable to sdkman, jabba, and jenv, while keeping the "no daemon" and "symlink-based" design invariants.

### Scope boundaries

- **In scope:** CLI subcommand `sjvm install`, TUI install screen, Adoptium (Temurin) and GraalVM CE downloads, SHA-256 verification, `.tar.gz` (Linux/macOS) and `.zip` (Windows) extraction, post-install "switch now?" prompt, memory cache invalidation.
- **Out of scope:** Support for Zulu, Corretto, Liberica, or other vendor distributions (can be added later by extending `jdk_catalog.rs`). Installing from a local archive file. Uninstall subcommand. Automatic OS/arch selection beyond the two supported auto-detected values.

---

## 2. CLI Specification

### 2.1 Subcommand signature

```
sjvm install <VERSION> [OPTIONS]

Arguments:
  <VERSION>   Major version number to install (e.g. "17", "21")

Options:
  --vendor <VENDOR>   JDK vendor [default: openjdk] [possible values: openjdk, graalvm]
  --os <OS>           Target OS override [default: auto-detected at runtime]
  --arch <ARCH>       Target architecture override [default: auto-detected at runtime]
  --force             Re-download and overwrite if already installed
  -h, --help          Print help
```

### 2.2 Argument table

| Argument | Type | Required | Default | Validation |
|---|---|---|---|---|
| `VERSION` | `String` | Yes | — | Passed through existing `validate_version` (alphanumeric, `-`, `.`, `_`; max 64 chars; non-empty). Additionally validated that the numeric portion is in range 8–25 by `validate_install_version` (see §2.3). |
| `--vendor` | `Vendor` enum | No | `openjdk` | Closed enum: `openjdk` or `graalvm`. Clap `ValueEnum` rejects unknown values at parse time. |
| `--os` | `String` | No | Runtime detect | When provided: lowercased, must match one of `linux`, `mac`, `windows`. Rejected otherwise. |
| `--arch` | `String` | No | Runtime detect | When provided: lowercased, must match one of `x64`, `aarch64`. Rejected otherwise. |
| `--force` | `bool` flag | No | `false` | No value; presence sets flag. |

### 2.3 Version validation rules

The `validate_install_version` function (in `src/commands/install.rs`) applies on top of the base `validate_version` from `main.rs`:

1. Parse the leading numeric portion of the version string (e.g. `"17"` from `"17"`, or the first segment of `"17.0.1"`).
2. If parsing succeeds, check `8 <= major <= 25`. Return `Err` if out of range.
3. If the string is non-numeric (e.g. `"temurin-17"`) it is passed through — the catalog layer resolves it.

### 2.4 OS and architecture auto-detection

When `--os` and `--arch` are not provided, they are detected at runtime by a small `detect_os()` / `detect_arch()` function in `src/core/jdk_catalog.rs`:

| `std::env::consts::OS` | Mapped os value |
|---|---|
| `"linux"` | `"linux"` |
| `"macos"` | `"mac"` |
| `"windows"` | `"windows"` |
| anything else | Error: unsupported OS |

| `std::env::consts::ARCH` | Mapped arch value |
|---|---|
| `"x86_64"` | `"x64"` |
| `"aarch64"` | `"aarch64"` |
| anything else | Error: unsupported architecture |

### 2.5 CLI examples

```bash
# Install the latest Temurin JDK 21 for the current platform
sjvm install 21

# Install GraalVM CE 17
sjvm install 17 --vendor graalvm

# Install on a different architecture (cross-download for distribution)
sjvm install 21 --os linux --arch aarch64

# Overwrite an existing installation
sjvm install 21 --force

# Version out of range → immediate error before any network call
sjvm install 99
# ❌ Version '99' is out of the supported range (8–25)

# Already installed (no --force) → warning, no download
sjvm install 21
# ⚠  JDK 21 (openjdk) is already installed at /usr/lib/jvm/temurin-21-amd64
#    Use --force to re-download and overwrite.
```

### 2.6 Post-install prompt

After a successful installation, if the process is attached to an interactive terminal (`std::io::IsTerminal::is_terminal(&std::io::stdin())`), the following prompt is printed to stdout and read from stdin:

```
✅ Installed temurin-21-amd64 → /usr/lib/jvm/temurin-21-amd64

Switch to the newly installed JDK now? [y/N]
```

- `y` or `Y` → call `switch_to_jdk` and print `✅ Now using JDK: ...`
- Any other input (including empty / Enter) → no switch; print nothing extra.
- Non-interactive (piped stdin or CI) → skip prompt entirely; do not print it.

### 2.7 Error cases and exit codes

| Condition | Message | Exit code |
|---|---|---|
| Version out of range | `❌ Version '{v}' is out of the supported range (8–25)` | 1 |
| Unknown OS (override) | `❌ Unsupported OS '{os}'. Valid values: linux, mac, windows` | 1 |
| Unknown arch (override) | `❌ Unsupported architecture '{arch}'. Valid values: x64, aarch64` | 1 |
| Already installed (no --force) | `⚠  JDK {v} ({vendor}) is already installed at {path}\n   Use --force to re-download and overwrite.` | 1 |
| API request failed | `❌ Failed to fetch JDK catalog: {reason}` | 1 |
| No artifact found | `❌ No JDK artifact found for {vendor} {version} {os}/{arch}` | 1 |
| Download failed | `❌ Download failed: {reason}` | 1 |
| Checksum mismatch | `❌ SHA-256 checksum mismatch — the downloaded file may be corrupted or tampered with` | 1 |
| Extraction failed | `❌ Archive extraction failed: {reason}` | 1 |
| Destination write failed | `❌ Failed to move JDK to installation directory: {reason}` | 1 |
| Symlink switch failed (post-install) | `❌ Installed but failed to switch: {reason}` | 1 |

All errors are printed to `stderr` and the process exits with code 1. Success exits with code 0.

---

## 3. TUI Specification

### 3.1 Overview

The existing `sjvm ui` command currently shows a single **Switch Screen**. This feature adds a second screen: the **Install Screen**. Both screens coexist in the TUI; the user switches between them with a keyboard shortcut. The TUI remains gated behind the `ui` feature flag.

### 3.2 File restructuring

The current monolithic `src/commands/ui.rs` (440 lines) is refactored into a module directory:

```
src/commands/ui/
├── mod.rs             ← Replaces ui.rs: App state, screen enum, event loop, tab navigation
├── switch_screen.rs   ← Extracted from ui.rs: JdkItem, SwitchApp, render_switch_screen
└── install_screen.rs  ← New: InstallApp, render_install_screen, inline progress
```

`src/commands/mod.rs` changes: replace `pub(crate) mod ui;` with `pub(crate) mod ui;` pointing at the directory (no code change needed — Rust resolves `mod ui` to either `ui.rs` or `ui/mod.rs` automatically).

The public surface is unchanged: `pub(crate) fn interactive_select() -> anyhow::Result<()>` remains in `ui/mod.rs`.

### 3.3 Screen enum and top-level state

```
enum Screen {
    Switch,
    Install,
}
```

The top-level `App` struct in `ui/mod.rs` holds:
- `screen: Screen` — active screen
- `switch: SwitchState` — owns the existing switch screen state
- `install: InstallState` — owns the new install screen state

### 3.4 Navigation between screens

| Key | Action |
|---|---|
| `Tab` | Toggle between Switch and Install screens |
| `i` | Jump directly to Install screen from Switch screen |
| `s` | Jump directly to Switch screen from Install screen |
| `q` / `Esc` | Quit the TUI entirely (from any screen) |

A tab bar at the top of the terminal area shows the active screen:

```
┌─────────────────────────────────────────────────────────────────┐
│  [ Switch ]  [ Install ]          SJVM - Java version manager   │
└─────────────────────────────────────────────────────────────────┘
```

Active tab is rendered with `Color::Cyan` background + `Modifier::BOLD`. Inactive tab is rendered with `Color::DarkGray` foreground.

### 3.5 Install screen layout

```
┌─────────────────────────────────────────────────────────────────┐
│  [ Switch ]  [ Install ]          SJVM - Java version manager   │  ← Tab bar (3 lines)
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  Vendor                                                         │  ← Vendor picker (5 lines)
│  >> OpenJDK (Temurin)                                           │
│     GraalVM CE                                                  │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  Version   (↑/↓ to move, Enter to install)                      │  ← Version list (fills remaining)
│  >> 21 (LTS)                                                    │
│     17 (LTS)                                                    │
│     11 (LTS)                                                    │
│      8 (LTS)                                                    │
│     23                                                          │
│     22                                                          │
│     ...                                                         │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  [Downloading temurin-21-amd64.tar.gz ████████░░░░░  63%  ]     │  ← Progress (3 lines, hidden until active)
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  Tab: switch screens  ↑/↓: navigate  Enter: confirm  q: quit    │  ← Help bar (3 lines)
└─────────────────────────────────────────────────────────────────┘
```

The progress bar area is always allocated (to prevent layout jitter) but is empty/blank until a download is in progress.

### 3.6 Install screen state machine

```
         ┌─────────────────────────────────────────────┐
         │                                             │
         ▼                                             │
   ┌──────────┐    Tab/Enter    ┌──────────────┐       │
   │  Idle    │ ─────────────► │ VendorPicker │       │
   │ (initial)│                │ OpenJDK/     │       │
   └──────────┘                │ GraalVM CE   │       │
                               └──────────────┘       │
                                      │ Enter         │
                                      ▼               │
                               ┌──────────────┐       │
                               │ VersionList  │       │
                               │ (8 → 25,     │ Esc   │
                               │  fetched     │ ──────┘
                               │  from API)   │
                               └──────────────┘
                                      │ Enter
                                      ▼
                               ┌──────────────┐
                               │ Downloading  │
                               │ (gauge bar   │
                               │  + % label)  │
                               └──────────────┘
                                   │        │
                              OK   │        │ Error
                                   ▼        ▼
                           ┌────────────┐ ┌──────────────┐
                           │ Installed  │ │   Failed     │
                           │ ✅ message │ │ ❌ message   │
                           │ + switch?  │ │              │
                           └────────────┘ └──────────────┘
                                 │ y/n         │ any key
                                 ▼             ▼
                              ┌────────────────────┐
                              │   Back to Idle     │
                              │   (version list    │
                              │    resets)         │
                              └────────────────────┘
```

**State variants for `InstallState`:**

| Variant | Description |
|---|---|
| `Idle` | Entry state; vendor picker is focused |
| `VendorPicker` | User is navigating between OpenJDK and GraalVM CE |
| `FetchingVersions` | Background thread is fetching the version list from the API |
| `VersionList` | Version list is loaded and user is navigating |
| `Downloading { progress: f64, label: String }` | Download in progress; `progress` is 0.0–1.0 |
| `Installed { jdk_path: PathBuf }` | Download + verify + extract succeeded |
| `Failed { message: String }` | Any step failed; message shown in red |

### 3.7 Progress rendering in TUI

The download is run on a **separate thread** (or via `std::thread::spawn`). Progress is communicated back to the TUI event loop via a `std::sync::mpsc::channel`:

```
TUI event loop thread            Downloader thread
       │                                 │
       │   Sender<DownloadEvent>         │
       │ ◄────────────────────────────── │ sends DownloadEvent::Progress(ratio, label)
       │                                 │ sends DownloadEvent::Done(PathBuf)
       │                                 │ sends DownloadEvent::Error(String)
       │
       │ each tick: drain channel,
       │ update InstallState::Downloading.progress
       │ re-render gauge widget
```

`DownloadEvent` enum:

```
enum DownloadEvent {
    Progress { downloaded: u64, total: u64 },
    Done { jdk_dir: PathBuf },
    Error { message: String },
}
```

The ratatui `Gauge` widget (from `ratatui::widgets::Gauge`) is used to render the progress bar. It receives a `ratio: f64` (0.0–1.0) derived from `downloaded / total`. The label is `"filename  downloaded_mb / total_mb"`.

The event loop polls the channel on every tick with a non-blocking `try_recv` before checking terminal events. Tick rate remains 100 ms (unchanged from existing loop).

### 3.8 Switch prompt inside TUI

After `InstallState::Installed` is reached, the progress bar area is replaced with:

```
✅ Installed temurin-21-amd64  →  Switch now? [y/N]
```

- `y` → calls `switch_to_jdk` (same function used by the Switch screen), updates `SwitchState`, transitions to `Switch` screen.
- `n` or any other key → transitions back to `Idle` on the Install screen.

### 3.9 Keybinding table — Install screen

| State | Key | Action |
|---|---|---|
| VendorPicker | `↑` / `k` | Previous vendor |
| VendorPicker | `↓` / `j` | Next vendor |
| VendorPicker | `Enter` | Confirm vendor, trigger API fetch, transition to `FetchingVersions` |
| FetchingVersions | — | No input accepted; spinner shown |
| VersionList | `↑` / `k` | Previous version |
| VersionList | `↓` / `j` | Next version |
| VersionList | `Enter` | Confirm version, spawn download thread, transition to `Downloading` |
| VersionList | `Esc` | Go back to VendorPicker |
| Downloading | — | No input accepted; display only |
| Installed | `y` / `Y` | Switch to installed JDK |
| Installed | any other | Back to `Idle` |
| Failed | any key | Back to `Idle` |
| Any | `Tab` | Switch to Switch screen |
| Any | `q` / `Esc` | Quit TUI |

---

## 4. Architecture

### 4.1 New module inventory

| File | Layer | Responsibility |
|---|---|---|
| `src/commands/install.rs` | `commands` | Thin CLI handler: parse args → call `jdk_catalog` → call `downloader` → prompt for switch |
| `src/commands/ui/mod.rs` | `commands` | Replaces `ui.rs`: holds screen enum, top-level `App`, event loop, tab bar rendering |
| `src/commands/ui/switch_screen.rs` | `commands` | Extracted from `ui.rs`: `SwitchState`, `render_switch_screen`, navigation |
| `src/commands/ui/install_screen.rs` | `commands` | New: `InstallState`, `render_install_screen`, channel receiver for download progress |
| `src/core/jdk_catalog.rs` | `core` | Pure catalog logic: `(vendor, version, os, arch)` → `ArtifactInfo { download_url, sha256_url, filename }`. Makes HTTP calls via `infra/http.rs`. |
| `src/core/downloader.rs` | `core` | Orchestrates: temp file → download (streaming) → SHA-256 verify → extract → move to `jdks_dirs[0]` → invalidate cache |
| `src/infra/http.rs` | `infra` | Single `reqwest` blocking client wrapper; centralizes TLS config and User-Agent header |

### 4.2 Module responsibility rules

- `jdk_catalog.rs` knows about vendor-specific API URLs and response shapes. It must not touch the filesystem.
- `downloader.rs` knows about filesystem operations (temp files, extraction, moves). It must not know about vendor APIs; it only receives an `ArtifactInfo` struct.
- `http.rs` only holds the HTTP client. No business logic.
- `install.rs` only orchestrates the above; it does not implement network or filesystem logic itself.
- `install_screen.rs` only renders and manages TUI state. It delegates all download logic to `downloader.rs` via the mpsc channel.

### 4.3 Key data types

**`ArtifactInfo`** (in `src/core/jdk_catalog.rs`):
```
pub(crate) struct ArtifactInfo {
    pub(crate) download_url: String,
    pub(crate) sha256_url: String,   // or sha256_value if the API returns it inline
    pub(crate) filename: String,
    pub(crate) vendor: Vendor,
    pub(crate) version: u8,
}
```

**`Vendor`** (in `src/core/jdk_catalog.rs`):
```
pub(crate) enum Vendor {
    OpenJdk,   // Adoptium Temurin
    GraalVm,   // GraalVM CE
}
```

`Vendor` also derives `clap::ValueEnum` so it can be used directly as a clap argument type in `install.rs`.

**`DownloadEvent`** (in `src/commands/ui/install_screen.rs`, `#[cfg(feature = "ui")]`):
```
pub(crate) enum DownloadEvent {
    Progress { downloaded: u64, total: u64 },
    Done { jdk_dir: PathBuf },
    Error { message: String },
}
```

**`InstallRequest`** (in `src/core/downloader.rs`):
```
pub(crate) struct InstallRequest {
    pub(crate) artifact: ArtifactInfo,
    pub(crate) dest_dir: PathBuf,       // jdks_dirs[0]
    pub(crate) force: bool,
}
```

### 4.4 Data flow — CLI path

```
main.rs
  │  Commands::Install { version, vendor, os, arch, force }
  ▼
commands/install.rs :: run_install()
  │
  ├─► core/jdk_catalog.rs :: resolve_artifact(vendor, version, os, arch)
  │       │
  │       └─► infra/http.rs :: get_json(url)
  │               │
  │               └─► Adoptium API / GitHub Releases API
  │                   returns: ArtifactInfo { download_url, sha256_url, filename }
  │
  ├─► [check if already installed in jdks_dirs[0]]
  │   ├─ found + !force → print warning, exit 1
  │   └─ found + force  → continue (downloader will overwrite)
  │
  └─► core/downloader.rs :: install_jdk(request, progress_cb)
          │
          ├─► infra/http.rs :: download_to_temp(url) → streaming, calls progress_cb(downloaded, total)
          ├─► infra/http.rs :: get_text(sha256_url) → expected_checksum: String
          ├─► [compute sha2::Sha256 of temp file]
          ├─► [compare; delete temp + bail! on mismatch]
          ├─► [extract .tar.gz or .zip from temp to temp_extract_dir]
          ├─► [rename top-level extracted dir → dest_dir/jdk-name]
          ├─► [delete temp file]
          ├─► [invalidate memory cache: delete memory_file()]
          └─► returns: PathBuf (installed JDK dir)

commands/install.rs
  └─► [post-install prompt] → optionally calls core/jdk_switcher.rs :: switch_to_jdk()
```

### 4.5 Data flow — TUI path

```
ui/mod.rs :: run_app_loop()
  │  (Tab or 'i' key)
  ▼
ui/install_screen.rs :: InstallState
  │  (user picks vendor + version, presses Enter)
  ▼
  std::thread::spawn {
      core/jdk_catalog.rs :: resolve_artifact(...)
      core/downloader.rs :: install_jdk(request, |ev| tx.send(ev))
  }
  │
  │  mpsc::Receiver<DownloadEvent>
  ▼
  each TUI tick: drain Receiver
    Progress   → update InstallState::Downloading.progress, re-render Gauge
    Done       → transition to InstallState::Installed
    Error      → transition to InstallState::Failed
  │
  │  user presses 'y'
  ▼
  core/jdk_switcher.rs :: switch_to_jdk()
  ui/switch_screen.rs :: SwitchState::reload()  ← refreshes the switch list
```

### 4.6 Integration with existing modules

| Existing module | Change |
|---|---|
| `main.rs` | Add `Commands::Install { ... }` variant and dispatch arm; add `use commands::install::run_install` |
| `commands/mod.rs` | Add `pub(crate) mod install;` (unconditional); change `pub(crate) mod ui;` to point at directory |
| `infra/memory.rs` | Add `pub(crate) fn invalidate_memory()` that deletes `memory_file()` from disk (cache is already in `OnceLock` — caller must restart or is done for this invocation) |
| `core/jdk_resolver.rs` | No change needed; detection runs from scratch next invocation |
| `infra/config.rs` | No change needed; `jdks_dirs[0]` is the install destination |

---

## 5. New Cargo Dependencies

### 5.1 Dependency table

| Crate | Version | Features | Purpose | Unconditional or feature-gated |
|---|---|---|---|---|
| `reqwest` | `0.12` | `blocking`, `rustls-tls`, `json` | HTTP downloads and API calls | Unconditional (install is always compiled) |
| `sha2` | `0.10` | — | SHA-256 checksum computation | Unconditional |
| `indicatif` | `0.17` | — | Progress bar for CLI download path | Unconditional |
| `tar` | `0.4` | — | `.tar.gz` extraction on Linux/macOS | Unconditional (guarded at runtime for Windows) |
| `flate2` | `1.0` | — | gzip decompression alongside `tar` | Unconditional |
| `zip` | `2.x` | — | `.zip` extraction on Windows | Unconditional (guarded at runtime for non-Windows) |

> **Note on `default-features = false`:** Enable only the features listed above for each crate. This minimizes the transitive dependency tree and reduces the attack surface, consistent with the project's security guidelines.

### 5.2 Justification

**`reqwest` with `rustls-tls` (not `native-tls`)**  
`native-tls` links against the OS TLS library (OpenSSL on Linux, Secure Transport on macOS, SChannel on Windows). This introduces:
- A system dependency that may not be present in minimal Docker or CI images.
- Version skew between the OS TLS library and the TLS version the code was tested against.
- `openssl-sys` as a transitive dependency, which has historically had build issues and supply-chain incidents.

`rustls-tls` compiles the TLS stack (rustls + webpki) directly into the binary. This gives:
- Zero system TLS dependency — the binary is fully self-contained.
- A pure-Rust TLS implementation that is auditable and subject to the same `cargo audit` / `cargo geiger` pipeline as the rest of the codebase.
- Consistent TLS behavior across all platforms without OS-specific configuration.

The downside is a slightly larger binary size; this is acceptable for a developer tool.

**`sha2`**  
The `sha2` crate from the RustCrypto project provides a pure-Rust, audited SHA-256 implementation. It is widely used, well-maintained, and has no unsafe code in the critical path. The alternative — calling a system `sha256sum` subprocess — would be fragile, platform-specific, and introduce a command injection surface.

**`indicatif`**  
Provides the download progress bar in the non-TUI (CLI) path. It handles terminal width detection, percentage display, download speed, and ETA without requiring manual ANSI escape code management. This is the de-facto standard progress bar crate for Rust CLIs.

**`tar` + `flate2`**  
These are the canonical Rust crates for `.tar.gz` extraction. They have no system dependencies, are pure Rust, and work identically across Linux, macOS, and Windows (for cross-platform extraction of Linux archives on Windows). The Adoptium API returns `.tar.gz` for Linux and macOS artifacts.

**`zip`**  
The canonical Rust `.zip` extraction crate, needed for Windows JDK artifacts which ship as `.zip`. Only used at runtime on Windows (guarded by `#[cfg(target_os = "windows")]` at the call site, but compiled on all platforms to avoid conditional compilation complexity in `Cargo.toml`).

### 5.3 `Cargo.toml` additions

```toml
# Always compiled — install subcommand is not feature-gated
reqwest  = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls", "json"] }
sha2     = { version = "0.10", default-features = false }
indicatif = { version = "0.17", default-features = false }
tar      = { version = "0.4",  default-features = false }
flate2   = { version = "1.0",  default-features = false, features = ["rust_backend"] }
zip      = { version = "2",    default-features = false, features = ["deflate"] }
```

> **Security pin note:** After adding these dependencies, run `cargo audit` and pin any crate with an active advisory in `Cargo.toml` with a comment referencing the advisory ID, following the pattern of the existing `time` pin.

---

## 6. API Integration

### 6.1 Adoptium (Temurin) API

**Base URL:** `https://api.adoptium.net`  
**Endpoint:** `GET /v3/assets/latest/{version}/hotspot`

**Full request URL example (Java 21, Linux x64):**
```
https://api.adoptium.net/v3/assets/latest/21/hotspot?os=linux&architecture=x64&image_type=jdk&jvm_impl=hotspot&vendor=eclipse
```

**Query parameters:**

| Parameter | Value | Notes |
|---|---|---|
| `os` | `linux` / `mac` / `windows` | Mapped from `detect_os()` or `--os` override |
| `architecture` | `x64` / `aarch64` | Mapped from `detect_arch()` or `--arch` override |
| `image_type` | `jdk` | Always `jdk`; we do not install JRE |
| `jvm_impl` | `hotspot` | Always `hotspot` |
| `vendor` | `eclipse` | Always `eclipse` for Temurin |

**Expected response:** JSON array. The first element contains the download information. Relevant fields:

```json
[
  {
    "binary": {
      "package": {
        "link": "https://github.com/adoptium/.../OpenJDK21U-jdk_x64_linux_hotspot_21.0.3_9.tar.gz",
        "checksum": "abc123...",
        "name": "OpenJDK21U-jdk_x64_linux_hotspot_21.0.3_9.tar.gz"
      }
    }
  }
]
```

**Parsing strategy:**
1. Deserialize to `serde_json::Value`.
2. Index `[0]["binary"]["package"]`.
3. Extract `link` (String), `checksum` (String, hex SHA-256), `name` (String).
4. If `checksum` is present and non-empty, use it directly (no separate SHA-256 URL request needed).
5. If `checksum` is absent or `null`, construct SHA-256 URL: `{link}.sha256.txt` and fetch separately.

**Error conditions:**
- Response is an empty array `[]` → no artifact found for that version/os/arch combination.
- HTTP 404 → version not available.
- HTTP 5xx → API outage; return error with retry suggestion.
- Malformed JSON or missing fields → return structured `Err` with field path context.

**Rate limiting:** The Adoptium API does not require authentication for read-only asset queries and has generous rate limits (hundreds of requests per minute). No special handling is required, but do not make multiple requests per `sjvm install` invocation (one request for catalog, one optional for SHA-256 — that's all).

### 6.2 GraalVM CE API (GitHub Releases)

**Base URL:** `https://api.github.com`  
**Endpoint:** `GET /repos/graalvm/graalvm-ce-builds/releases`

**Request:**
```
GET https://api.github.com/repos/graalvm/graalvm-ce-builds/releases
Accept: application/vnd.github+json
User-Agent: sjvm/{version} (https://github.com/fefou/sjvm)
```

> **Important:** GitHub API requires a `User-Agent` header. The `http.rs` wrapper must set this header on every request. Not setting it results in HTTP 403.

**Authentication:** None required for public releases. GitHub's unauthenticated rate limit is 60 requests/hour per IP, which is sufficient for this use case. If a `GITHUB_TOKEN` environment variable is set, the http wrapper should include it as `Authorization: Bearer {token}` to increase the rate limit — but it must never be hardcoded or logged.

**Filtering strategy:**
1. Fetch all releases (first page; default 30 per page is sufficient for recent releases).
2. Filter releases where `tag_name` matches `vm-{version}` (e.g. `vm-17`, `vm-21`).
3. From the matched release's `assets` array, find the asset whose `name` matches the pattern for the requested `os` and `arch`:
   - Linux x64: `graalvm-ce-java{version}-linux-amd64-{release}.tar.gz`
   - Linux aarch64: `graalvm-ce-java{version}-linux-aarch64-{release}.tar.gz`
   - macOS x64: `graalvm-ce-java{version}-darwin-amd64-{release}.tar.gz`
   - macOS aarch64: `graalvm-ce-java{version}-darwin-aarch64-{release}.tar.gz`
   - Windows x64: `graalvm-ce-java{version}-windows-amd64-{release}.zip`
4. Extract `browser_download_url` from the matched asset.
5. Look for a sibling asset named `{matched_name}.sha256` to get the checksum file URL.

**Error conditions:**
- No release with `tag_name == "vm-{version}"` → version not available for GraalVM CE.
- No asset matching the os/arch pattern → platform not available for this release.
- HTTP 403 → rate limit hit; suggest setting `GITHUB_TOKEN` env var.
- HTTP 404 → repository not found (unlikely; treat as API error).

**Pagination consideration:** If the target version is older than 30 releases, the first page may not contain it. Implement pagination: follow the `Link: <url>; rel="next"` response header (parse it from the raw header value) until the target version is found or pages are exhausted. Cap at 5 pages to avoid excessive requests.

### 6.3 HTTP client configuration (`infra/http.rs`)

The `http.rs` module exposes these `pub(crate)` functions only:

| Function | Signature | Description |
|---|---|---|
| `get_json` | `(url: &str) -> anyhow::Result<serde_json::Value>` | GET, parse JSON body |
| `get_text` | `(url: &str) -> anyhow::Result<String>` | GET, return body as String (for SHA-256 files) |
| `download_streaming` | `(url: &str, dest: &Path, progress: impl Fn(u64, u64)) -> anyhow::Result<()>` | GET, stream bytes to file, call `progress(downloaded, total)` on each chunk |

**Client initialization:**
- Use `reqwest::blocking::Client` (blocking feature, not async — keeps the codebase single-threaded outside the TUI download thread).
- Set `use_rustls_tls()`.
- Set `User-Agent: sjvm/{crate_version} (https://github.com/fefou/sjvm)` — derive the version from `env!("CARGO_PKG_VERSION")`.
- Set `timeout(Duration::from_secs(30))` for catalog requests (`get_json`, `get_text`).
- Set `timeout(Duration::from_secs(600))` (10 minutes) for `download_streaming` — large JDK archives can be 200+ MB.
- Store the client in a `OnceLock<reqwest::blocking::Client>` for reuse.

**TLS certificate validation:** Never disable certificate validation. No `danger_accept_invalid_certs` or similar calls. All HTTPS connections must fully validate the server certificate chain.

---

## 7. Download and Install Flow

### 7.1 Step-by-step flow

```
Step 1: Resolve artifact
    Input: (vendor, version_str, os, arch)
    Action: Call jdk_catalog::resolve_artifact() → ArtifactInfo
    On error: return Err (caller prints "❌ Failed to fetch JDK catalog: ...")

Step 2: Check existing installation
    Input: ArtifactInfo.filename, config().jdks_dirs[0]
    Action: Check if dest_path = Path::new(&config().jdks_dirs[0]).join(jdk_dir_name) exists as a directory
    If exists AND force=false: return Err("already installed — use --force")
    If exists AND force=true: continue; downloader will overwrite in step 6

Step 3: Create temp file
    Location: std::env::temp_dir().join(format!("sjvm-download-{}.tmp", artifact.filename))
    Validate: temp dir must not be inside any jdks_dirs (sanity check)
    On error: return Err("Failed to create temp download path: ...")

Step 4: Download to temp file (streaming)
    Action: infra/http.rs :: download_streaming(url, &temp_path, progress_cb)
    The progress_cb is:
        - CLI path: updates indicatif ProgressBar
        - TUI path: sends DownloadEvent::Progress { downloaded, total } over mpsc channel
    Chunk size: let reqwest handle; do not buffer entire file in memory
    On error: DELETE temp file, return Err("Download failed: ...")

Step 5: Fetch and verify SHA-256 checksum
    5a. Obtain expected checksum:
        - If ArtifactInfo.sha256_value is Some(s): use s directly
        - If ArtifactInfo.sha256_url is Some(url): call http::get_text(url), trim whitespace, take first token (some SHA files have "hash  filename" format)
    5b. Compute actual checksum:
        - Open temp file, read in 64 KiB chunks, feed into sha2::Sha256 digest
        - Finalize to hex string
    5c. Compare (constant-time is not required here; this is data integrity, not authentication):
        - If mismatch: DELETE temp file, return Err("SHA-256 checksum mismatch")
        - If match: continue

Step 6: Extract archive
    6a. Detect archive type from filename extension:
        - Ends with ".tar.gz" → use tar + flate2
        - Ends with ".zip" → use zip crate
        - Otherwise: DELETE temp file, return Err("Unrecognized archive format: ...")
    6b. Create temp extraction directory:
        std::env::temp_dir().join(format!("sjvm-extract-{}-{}", artifact.filename, process::id()))
    6c. Extract all entries into temp extraction dir
        - For tar: iterate archive.entries(), skip entries with ".." in path components (path traversal guard)
        - For zip: iterate zip.file_names(), reject any with ".." components
        - On extraction error: DELETE temp file AND temp_extract_dir, return Err("Archive extraction failed: ...")
    6d. Identify the top-level JDK directory:
        - Read entries of temp_extract_dir; there should be exactly one subdirectory
        - If zero or more than one top-level entry: clean up, return Err("Unexpected archive layout: expected a single top-level JDK directory")

Step 7: Move JDK to destination
    Source: temp_extract_dir/jdk-top-level-dir
    Destination: PathBuf::from(&config().jdks_dirs[0]).join(jdk_top_level_dir_name)
    Action: std::fs::rename(source, dest)
        - If rename fails across filesystems (ErrorKind::CrossesDevices or similar):
          Fall back to recursive copy + delete source
    Validate destination is inside jdks_dirs[0] (path traversal guard) before rename
    Delete temp_extract_dir after move
    Delete temp file (should already be deleted after extraction but ensure cleanup)

Step 8: Invalidate memory cache
    Action: infra/memory::invalidate_memory() → delete memory_file() from disk
    This forces the next sjvm command to re-scan jdks_dirs and rebuild the cache
    The OnceLock in the current process is already populated but the install
    subcommand is done after this step, so stale in-process state is harmless

Step 9: Return success
    Return: Ok(dest_path)  (the installed JDK directory)
```

### 7.2 Cleanup invariant

**The invariant:** at no point should a partial or corrupt JDK be left in `jdks_dirs[0]`. The sequence `temp → verify → extract to temp → move to dest` ensures that `dest` is only created upon full success. If any step before step 7 fails, only temp files are cleaned up; `jdks_dirs[0]` is never touched (except in the `--force` overwrite case where step 6 extracts to temp first and only moves after success).

### 7.3 Failure cleanup table

| Step that fails | Files to clean up |
|---|---|
| Step 4 (download) | Delete `temp_path` |
| Step 5 (checksum) | Delete `temp_path` |
| Step 6 (extract) | Delete `temp_path`, delete `temp_extract_dir` (recursive) |
| Step 7 (move) | Delete `temp_path`, delete `temp_extract_dir` (recursive) |

Cleanup failures (e.g. unable to delete temp file) are logged to `stderr` as warnings but do not change the primary error returned.

---

## 8. Security Considerations

### 8.1 Input validation

**Version argument:** Validated by the existing `validate_version` parser (alphanumeric, `-`, `.`, `_`; max 64 chars) plus the new numeric range check (8–25). This prevents injection into URL path segments via the version string.

**`--vendor` argument:** Restricted to a closed `ValueEnum` (`openjdk`, `graalvm`). Clap rejects all other values before any code runs.

**`--os` / `--arch` arguments:** Validated against a fixed allowlist in `resolve_artifact()`. Invalid values return `Err` immediately before any network call.

**All user inputs are validated at the CLI parse layer** (clap `value_parser`) so that business logic never receives unvalidated strings.

### 8.2 Path traversal prevention

Three separate layers:

1. **URL construction:** Version and os/arch values are validated to contain no `..`, `/`, `\`, or `%` sequences before being interpolated into API URLs.

2. **Archive extraction (tar and zip):** Each entry path is checked for `..` components using `Path::components()` before writing any file to disk. Entries with absolute paths (starting with `/` or a Windows drive letter) are also rejected. Any violation returns `Err` immediately and triggers full cleanup.

3. **Destination path verification:** After constructing the destination path (`jdks_dirs[0]/jdk-dir-name`), verify that the canonical destination starts with the canonical `jdks_dirs[0]` before performing the move. This mirrors the existing guard in `jdk_switcher.rs::switch_to_jdk`.

### 8.3 Checksum enforcement

SHA-256 verification is **mandatory and hard-failing**:
- If the expected checksum cannot be retrieved (API error, missing field), the download is **aborted** — we do not install without a checksum.
- If the computed checksum does not match, the temp file is deleted and `Err` is returned with the message `"SHA-256 checksum mismatch — the downloaded file may be corrupted or tampered with"`. The actual hash values are NOT printed (they provide no useful information to the user and lengthen output).

### 8.4 TLS

- `reqwest` is configured with `use_rustls_tls()` exclusively. Native TLS is never used.
- Certificate validation is always enabled. No calls to `danger_accept_invalid_certs(true)`.
- Download URLs received from the catalog APIs must use `https://`. If a non-HTTPS URL is received (which would indicate API compromise or mis-parsing), return `Err("Refusing to download over non-HTTPS URL")` before making any request.

### 8.5 Secrets

- The optional `GITHUB_TOKEN` environment variable, if read, must **never** appear in:
  - Error context strings (`.context("...")`)
  - Log output
  - `--help` text (use `hide_env_values = true` on the clap argument)
- No secrets are stored to disk.

### 8.6 Temp file permissions

On Unix, set `0o600` permissions on the temp download file immediately after creating it (before writing any data), using `std::fs::set_permissions`. This prevents other processes running under other users from reading the archive while it is being downloaded.

### 8.7 Supply chain

After adding new dependencies, the following must be run and pass before merging:
- `cargo audit` — no known CVEs
- `cargo deny check` — license and ban policy
- `cargo geiger` — review any new unsafe code introduced by transitive deps

---

## 9. Error Handling Catalogue

All errors use `anyhow::Result<T>` and include `.context()` / `.with_context()` at every fallible boundary. The following table covers every expected error case.

### 9.1 Input / CLI errors (caught at parse time, before any I/O)

| ID | Condition | Handling |
|---|---|---|
| E01 | Version string empty | `validate_version` in `main.rs` returns `Err`; clap prints error + help |
| E02 | Version string too long (> 64 chars) | Same as E01 |
| E03 | Version string contains illegal chars | Same as E01 |
| E04 | Major version out of range (< 8 or > 25) | `validate_install_version` returns `Err`; clap prints error |
| E05 | Unknown `--vendor` value | Clap `ValueEnum` rejects at parse time |
| E06 | Unknown `--os` value | `resolve_artifact` returns `Err("Unsupported OS...")` |
| E07 | Unknown `--arch` value | `resolve_artifact` returns `Err("Unsupported architecture...")` |

### 9.2 Catalog / API errors

| ID | Condition | Handling |
|---|---|---|
| E10 | Network unreachable | `reqwest` error → `.context("request to Adoptium API failed")`; printed as `❌ Failed to fetch JDK catalog: ...` |
| E11 | DNS resolution failure | Same as E10 |
| E12 | TLS handshake failure | Same as E10 |
| E13 | HTTP 404 from Adoptium API | `Err("No JDK artifact found for {vendor} {version} {os}/{arch}")` |
| E14 | HTTP 403 from GitHub API | `Err("GitHub API rate limit hit. Set GITHUB_TOKEN env var to increase the limit.")` |
| E15 | HTTP 5xx from either API | `Err("API server error (HTTP {status}). Please try again later.")` |
| E16 | Empty result array from Adoptium | `Err("No artifact found for ...")` |
| E17 | No matching release for GraalVM version | `Err("GraalVM CE {version} is not available from GitHub Releases")` |
| E18 | No matching asset for os/arch in GraalVM release | `Err("No GraalVM CE artifact found for {os}/{arch}")` |
| E19 | Malformed JSON / missing expected field | `.with_context(|| format!("Unexpected API response format at field '{field}': ..."))` |
| E20 | Checksum URL request fails | `Err("Failed to fetch SHA-256 checksum: ...")` |

### 9.3 Download errors

| ID | Condition | Handling |
|---|---|---|
| E30 | Temp directory not writable | `Err("Cannot write to temp directory: ...")` |
| E31 | Download stream interrupted | Delete temp file, `Err("Download interrupted: ...")` |
| E32 | Content-Length mismatch (received fewer bytes) | Delete temp file, `Err("Download incomplete: expected {n} bytes, got {m}")` |
| E33 | Non-HTTPS download URL | `Err("Refusing to download over non-HTTPS URL: {url}")` — no temp file created |

### 9.4 Verification errors

| ID | Condition | Handling |
|---|---|---|
| E40 | SHA-256 checksum mismatch | Delete temp file, `Err("SHA-256 checksum mismatch — ...")` |
| E41 | Expected checksum unavailable (both inline and URL absent) | Delete temp file, `Err("No checksum available for this artifact — aborting for security")` |
| E42 | Checksum file malformed (not a valid hex string) | Delete temp file, `Err("Checksum file did not contain a valid SHA-256 hex string")` |

### 9.5 Extraction errors

| ID | Condition | Handling |
|---|---|---|
| E50 | Unrecognized archive format | Delete temp file, `Err("Unrecognized archive format: expected .tar.gz or .zip")` |
| E51 | Archive entry contains `..` path component | Delete temp file + extract dir, `Err("Archive contains a path traversal entry — aborting")` |
| E52 | Archive entry has absolute path | Same as E51 |
| E53 | I/O error during extraction | Delete temp file + extract dir, `Err("Archive extraction failed: {reason}")` |
| E54 | Archive has zero or multiple top-level dirs | Clean up, `Err("Unexpected archive layout: expected exactly one top-level JDK directory")` |

### 9.6 Installation errors

| ID | Condition | Handling |
|---|---|---|
| E60 | Destination already exists + `--force` not set | `Err("JDK already installed at {path}. Use --force to overwrite.")` — no temp files created |
| E61 | Rename fails (same filesystem) | Clean up temp, `Err("Failed to move JDK to installation directory: ...")` |
| E62 | Cross-filesystem rename falls back to copy; copy fails | Clean up temp + partial dest, `Err("Failed to copy JDK to installation directory: ...")` |
| E63 | Destination path escapes `jdks_dirs[0]` | Clean up temp, `Err("Computed installation path is outside configured jdks_dirs — aborting")` |
| E64 | Memory cache invalidation fails (cannot delete `sjvm-mem`) | **Warning only** (non-fatal): `eprintln!("sjvm: WARNING — could not invalidate cache: {reason}. Run 'sjvm setup' to rebuild.")` |

### 9.7 Post-install switch errors

| ID | Condition | Handling |
|---|---|---|
| E70 | Switch fails after install (CLI) | `eprintln!("❌ Installed but failed to switch: {reason}")`, exit 1 |
| E71 | Switch fails after install (TUI) | Transition to `InstallState::Failed` with message "Installed, but could not switch: {reason}" |

---

## 10. Testing Plan

### 10.1 Guiding principles

Follow the project's testing guidelines: pure functions accept explicit parameters and are tested with `#[cfg(test)]` modules co-located in source files. No mocking framework. Filesystem tests use `std::env::temp_dir()`. TUI tests use `ratatui::backend::TestBackend`. No global `OnceLock` state is relied upon in unit tests.

### 10.2 `src/core/jdk_catalog.rs`

All tests operate on explicit input data or stub HTTP responses. HTTP calls are never made in unit tests.

| Test | What it covers |
|---|---|
| `test_detect_os_linux` | `std::env::consts::OS == "linux"` maps to `"linux"` |
| `test_detect_os_macos` | maps to `"mac"` |
| `test_detect_arch_x86_64` | `"x86_64"` maps to `"x64"` |
| `test_detect_arch_aarch64` | `"aarch64"` maps to `"aarch64"` |
| `test_parse_adoptium_response_valid` | Parse a hardcoded JSON blob matching the Adoptium response shape; assert `ArtifactInfo.download_url` and `sha256_value` are extracted correctly |
| `test_parse_adoptium_response_empty_array` | Empty array `[]` returns `Err` |
| `test_parse_adoptium_response_missing_checksum` | Missing `checksum` field falls back to SHA-256 URL construction |
| `test_parse_adoptium_response_malformed` | Missing `binary.package.link` field returns `Err` with context |
| `test_parse_graalvm_releases_finds_correct_tag` | Stub JSON with multiple releases; assert the `vm-21` release is selected |
| `test_parse_graalvm_releases_no_matching_tag` | No matching tag returns `Err` |
| `test_parse_graalvm_asset_linux_x64` | Correct asset is selected from release assets array |
| `test_parse_graalvm_asset_not_found_for_arch` | No matching asset returns `Err` |
| `test_url_validation_rejects_http` | `http://` URL returns `Err("non-HTTPS")` |
| `test_url_validation_accepts_https` | `https://` URL passes |

**How to test without making real HTTP calls:** Extract pure parsing functions (`parse_adoptium_response(json: &Value)`, `parse_graalvm_releases(json: &Value, version: u8, os: &str, arch: &str)`) that accept a `serde_json::Value`. Tests call these directly with hardcoded JSON fixtures. The `resolve_artifact` function (which makes actual HTTP calls) is not unit tested; it is covered by E2E tests in Docker.

### 10.3 `src/core/downloader.rs`

| Test | What it covers |
|---|---|
| `test_checksum_verify_match` | Feed known bytes through sha2, compare to expected hex string — succeeds |
| `test_checksum_verify_mismatch` | Wrong expected hex returns `Err("checksum mismatch")` |
| `test_checksum_verify_malformed_hex` | Non-hex expected string returns `Err("not a valid SHA-256 hex string")` |
| `test_extract_tar_gz_rejects_path_traversal` | A `.tar.gz` archive with `../../evil` entry returns `Err("path traversal")` — use `tar::Builder` to construct the fixture in memory |
| `test_extract_tar_gz_rejects_absolute_path` | Archive with `/etc/passwd` entry returns `Err` |
| `test_extract_tar_gz_valid` | Archive with `jdk-21/release` extracts correctly to temp dir |
| `test_extract_zip_rejects_path_traversal` | `.zip` with `../evil` entry returns `Err` |
| `test_extract_zip_valid` | Valid `.zip` extracts to temp dir |
| `test_identify_top_level_dir_single` | Temp dir with one subdirectory returns its name correctly |
| `test_identify_top_level_dir_multiple` | Temp dir with two subdirectories returns `Err("unexpected archive layout")` |
| `test_identify_top_level_dir_empty` | Empty temp dir returns `Err` |
| `test_destination_path_within_jdks_dir` | `jdks_dirs[0]/jdk-21` passes the path containment check |
| `test_destination_path_escapes_jdks_dir` | `jdks_dirs[0]/../etc` returns `Err("outside configured jdks_dirs")` |

**Filesystem fixtures:** Use `std::env::temp_dir()` + unique subdirectory per test (using `std::process::id()` or a counter) to avoid test interference. Always clean up in the test (even on failure — use a guard struct with `Drop`).

### 10.4 `src/infra/http.rs`

Unit tests do not make real HTTP calls. Tests cover only the client builder logic:

| Test | What it covers |
|---|---|
| `test_client_builds_without_panic` | `build_client()` returns `Ok` |
| `test_user_agent_header_is_set` | Inspect the default headers on the client; assert `User-Agent` contains `"sjvm/"` |

Integration tests for actual HTTP behavior belong in E2E / Docker tests only.

### 10.5 `src/commands/install.rs`

| Test | What it covers |
|---|---|
| `test_validate_install_version_accepts_8_to_25` | `8`, `17`, `21`, `25` all return `Ok` |
| `test_validate_install_version_rejects_below_8` | `7` returns `Err` |
| `test_validate_install_version_rejects_above_25` | `26` returns `Err` |
| `test_validate_install_version_non_numeric` | `"temurin-17"` passes through (numeric parse fails gracefully, no range check) |
| `test_install_command_parses_with_clap` | `Cli::try_parse_from(["sjvm", "install", "21"])` succeeds |
| `test_install_command_parses_vendor_graalvm` | `["sjvm", "install", "17", "--vendor", "graalvm"]` sets `vendor = Vendor::GraalVm` |
| `test_install_command_parses_force_flag` | `["sjvm", "install", "21", "--force"]` sets `force = true` |
| `test_install_command_rejects_unknown_vendor` | `["sjvm", "install", "21", "--vendor", "zulu"]` → `is_err()` |
| `test_install_already_installed_no_force` | Given a pre-existing dir at `jdks_dirs[0]/jdk-21`, `run_install` with `force=false` returns `Err("already installed")` without making any HTTP calls |

### 10.6 `src/commands/ui/install_screen.rs` (`#[cfg(feature = "ui")]`)

| Test | What it covers |
|---|---|
| `test_install_screen_renders_vendor_picker` | `TestBackend` renders the install screen in `VendorPicker` state; buffer contains "OpenJDK" and "GraalVM CE" |
| `test_install_screen_renders_downloading_gauge` | State = `Downloading { progress: 0.5, label: "..." }`; buffer contains the gauge characters |
| `test_install_screen_renders_installed_message` | State = `Installed { ... }`; buffer contains "✅ Installed" |
| `test_install_screen_renders_failed_message` | State = `Failed { message: "network error" }`; buffer contains "❌" and "network error" |
| `test_install_state_vendor_navigation` | `next_vendor()` / `prev_vendor()` cycles through `[OpenJDK, GraalVm]` |
| `test_install_state_version_navigation` | `next_version()` / `prev_version()` on a populated list wraps correctly |
| `test_download_event_progress_updates_state` | Feeding `DownloadEvent::Progress { downloaded: 50, total: 100 }` into state sets `progress = 0.5` |
| `test_download_event_done_transitions_state` | `DownloadEvent::Done { jdk_dir }` transitions from `Downloading` to `Installed` |
| `test_download_event_error_transitions_state` | `DownloadEvent::Error { message }` transitions to `Failed` |

### 10.7 `src/commands/ui/mod.rs` (TUI orchestration, `#[cfg(feature = "ui")]`)

| Test | What it covers |
|---|---|
| `test_tab_bar_renders_switch_active` | Screen = `Switch`; buffer contains highlighted "Switch" tab |
| `test_tab_bar_renders_install_active` | Screen = `Install`; buffer contains highlighted "Install" tab |
| `test_tab_key_toggles_screen` | `process_key(Tab)` flips `App.screen` between `Switch` and `Install` |
| `test_i_key_jumps_to_install` | From `Switch` screen, `process_key('i')` sets `screen = Install` |
| `test_s_key_jumps_to_switch` | From `Install` screen, `process_key('s')` sets `screen = Switch` |

### 10.8 E2E tests (Docker only — added to `tests/e2e.rs`)

These tests run only in the Docker environment with real network access:

| Test | What it covers |
|---|---|
| `test_e2e_install_temurin_21` | `sjvm install 21` downloads, verifies, extracts, installs Temurin 21; asserts directory exists in `jdks_dirs[0]`; asserts memory cache is rebuilt on next `sjvm list` |
| `test_e2e_install_already_installed_no_force` | Second `sjvm install 21` exits with code 1 and prints the "already installed" warning |
| `test_e2e_install_already_installed_force` | `sjvm install 21 --force` re-downloads and succeeds |
| `test_e2e_install_graalvm_17` | `sjvm install 17 --vendor graalvm` installs GraalVM CE 17 |

---

## 11. Implementation Tasks

The tasks below are ordered for a single developer. Tasks within the same numbered group can be parallelized if multiple contributors are available. Each task is independently compilable and testable.

### Phase 1: Infrastructure (no UI, no install logic yet)

- [ ] **T01** — Add new Cargo dependencies to `Cargo.toml`: `reqwest`, `sha2`, `indicatif`, `tar`, `flate2`, `zip`. Use `default-features = false` and only the specified feature flags. Run `cargo check --all-features` to verify compilation. Run `cargo audit` and address any advisories.

- [ ] **T02** — Create `src/infra/http.rs`. Implement `build_client()` returning a `OnceLock`-backed `reqwest::blocking::Client` with `rustls-tls`, custom `User-Agent`, and timeouts. Implement `get_json(url)`, `get_text(url)`, and `download_streaming(url, dest, progress_cb)`. Add `pub(crate) mod http;` to `src/infra/mod.rs`. Write unit tests for client construction and User-Agent header.

- [ ] **T03** — Add `pub(crate) fn invalidate_memory() -> anyhow::Result<()>` to `src/infra/memory.rs`. This function deletes `memory_file()` from disk; if the file does not exist, it is a no-op (success). Add a unit test: create a temp file, call `invalidate_memory` pointing at it (inject path), verify file is deleted.

### Phase 2: Core catalog and downloader logic

- [ ] **T04** — Create `src/core/jdk_catalog.rs`. Define `Vendor` enum (with `clap::ValueEnum` derive and `serde` derives for display), `ArtifactInfo` struct. Implement `detect_os()`, `detect_arch()`. Implement `parse_adoptium_response(json: &Value) -> anyhow::Result<ArtifactInfo>` and `parse_graalvm_releases(json: &Value, version: u8, os: &str, arch: &str) -> anyhow::Result<ArtifactInfo>` as pure, testable functions. Implement `resolve_artifact(vendor, version, os, arch) -> anyhow::Result<ArtifactInfo>` which calls `http::get_json` and delegates to the parsers. Add `pub(crate) mod jdk_catalog;` to `src/core/mod.rs`. Write all unit tests from §10.2.

- [ ] **T05** — Create `src/core/downloader.rs`. Define `InstallRequest` struct. Implement helper functions: `verify_checksum(file_path, expected_hex) -> anyhow::Result<()>`, `extract_tar_gz(archive_path, dest_dir) -> anyhow::Result<()>` (with path traversal guard), `extract_zip(archive_path, dest_dir) -> anyhow::Result<()>` (with path traversal guard), `identify_top_level_dir(extract_dir) -> anyhow::Result<PathBuf>`, `validate_dest_within_jdks_dir(dest, jdks_dir) -> anyhow::Result<()>`. Implement `install_jdk(request: InstallRequest, progress: impl Fn(u64, u64)) -> anyhow::Result<PathBuf>` as the main orchestration function (steps 3–8 from §7.1). Add `pub(crate) mod downloader;` to `src/core/mod.rs`. Write all unit tests from §10.3.

### Phase 3: CLI install command

- [ ] **T06** — Create `src/commands/install.rs`. Define `validate_install_version(s: &str) -> Result<String, String>` (range check). Implement `run_install(version, vendor, os, arch, force) -> anyhow::Result<()>`: check existing installation, call `resolve_artifact`, call `install_jdk` with an `indicatif::ProgressBar` as the progress callback, call `invalidate_memory`, display post-install prompt, optionally call `switch_to_jdk`. Add `pub(crate) mod install;` to `src/commands/mod.rs`. Write unit tests from §10.5.

- [ ] **T07** — Wire `Commands::Install` into `src/main.rs`. Add the `Install { version, vendor, os, arch, force }` variant to the `Commands` enum. Add the dispatch arm in `main()` calling `run_install(...)`. Use `#[derive(clap::ValueEnum)]` on `Vendor`. Write clap parsing tests from §10.5 using `Cli::try_parse_from`.

- [ ] **T08** — Manual smoke test (non-Docker): Run `sjvm install 99` and verify range error. Run `sjvm install 21 --vendor zulu` and verify clap rejects it. Run `sjvm install 21 --os bsd` and verify unsupported OS error. (No actual download required for these tests.)

### Phase 4: TUI refactor and install screen

> **Prerequisite:** Phases 1–3 complete and all unit tests passing.

- [ ] **T09** — Refactor `src/commands/ui.rs` into a module directory. Create `src/commands/ui/` directory. Move existing content of `ui.rs` into `ui/mod.rs` — no logic changes, only file reorganization. Extract the `App`/`JdkItem`/`render_ui`/`run_app_loop` code related to the switch screen into `ui/switch_screen.rs` as `SwitchState` and `render_switch_screen`. Update `ui/mod.rs` to use `switch_screen::SwitchState`. Verify all existing TUI unit tests pass unchanged after refactor. Delete `src/commands/ui.rs`.

- [ ] **T10** — Add screen enum and tab bar. In `ui/mod.rs`: add `enum Screen { Switch, Install }`. Add `screen: Screen` to the top-level `App` struct. Add `install: InstallState` placeholder (empty struct for now). Implement `render_tab_bar(f, area, active_screen)`. Add Tab / `i` / `s` key handling to the event loop. Write unit tests from §10.7 (tab bar rendering and key handling).

- [ ] **T11** — Implement `src/commands/ui/install_screen.rs`. Define `DownloadEvent`, `InstallState` enum with all variants. Implement `render_install_screen(f: &mut Frame, area: Rect, state: &mut InstallState)` covering all state variants (vendor picker, version list, gauge, result messages). Write all unit tests from §10.6 using `TestBackend`.

- [ ] **T12** — Wire download into TUI. In `install_screen.rs`, implement `spawn_download(artifact: ArtifactInfo, dest_dir: PathBuf, tx: Sender<DownloadEvent>)` that spawns a thread calling `downloader::install_jdk` and sends `DownloadEvent` messages. In `ui/mod.rs` event loop, add `Receiver<DownloadEvent>` draining on each tick. Handle `Done` and `Error` events by transitioning `InstallState`. Handle `y` key in `Installed` state by calling `switch_to_jdk` and reloading `SwitchState`.

- [ ] **T13** — Wire API fetching into TUI. On `Enter` in `VendorPicker` state, spawn a thread to call `jdk_catalog::resolve_artifact` (or a simpler version list fetch), send results back via a separate channel, transition through `FetchingVersions` to `VersionList`. Implement a spinner or "Fetching..." placeholder in `FetchingVersions` state. Write or update unit tests for this state transition.

### Phase 5: Validation and cleanup

- [ ] **T14** — Run full test suite: `cargo test --all-features`. Fix any regressions. Run `cargo clippy --all-features -- -D warnings`. Fix all warnings. Run `cargo fmt --check`.

- [ ] **T15** — Run `cargo audit`. If any new advisories are found in the new dependencies, pin the affected crate with a comment referencing the advisory ID (following the pattern of the existing `time` pin in `Cargo.toml`).

- [ ] **T16** — Update `Cargo.lock` (committed, as required for this binary). Verify `cargo build --release` succeeds and produces a working binary.

- [ ] **T17** — Add E2E tests to `tests/e2e.rs` (Docker-only, `#[ignore]`): `test_e2e_install_temurin_21`, `test_e2e_install_already_installed_no_force`, `test_e2e_install_already_installed_force`. Mark them `#[ignore]` so they are skipped by `cargo test` and only run explicitly in Docker.

- [ ] **T18** — Update Docker environment if needed: ensure the Docker `it-ubuntu-compose.yaml` configuration gives the container network access for the E2E install tests, and that `jdks_dirs` is configured to a writable directory inside the container.

---

## Appendix A: File change summary

| File | Change type | Notes |
|---|---|---|
| `Cargo.toml` | Modified | Add 6 new dependencies |
| `Cargo.lock` | Modified | Updated by cargo |
| `src/main.rs` | Modified | Add `Install` command variant and dispatch arm |
| `src/commands/mod.rs` | Modified | Add `pub(crate) mod install;` |
| `src/commands/install.rs` | New | CLI handler |
| `src/commands/ui.rs` | Deleted | Replaced by `ui/` module directory |
| `src/commands/ui/mod.rs` | New | Replaces `ui.rs`; adds screen enum, tab bar, event loop |
| `src/commands/ui/switch_screen.rs` | New | Extracted from `ui.rs` |
| `src/commands/ui/install_screen.rs` | New | New install screen |
| `src/core/mod.rs` | Modified | Add `pub(crate) mod jdk_catalog;` and `pub(crate) mod downloader;` |
| `src/core/jdk_catalog.rs` | New | API catalog logic |
| `src/core/downloader.rs` | New | Download + verify + extract pipeline |
| `src/infra/mod.rs` | Modified | Add `pub(crate) mod http;` |
| `src/infra/http.rs` | New | reqwest blocking client wrapper |
| `src/infra/memory.rs` | Modified | Add `invalidate_memory()` |
| `tests/e2e.rs` | Modified | Add 4 new E2E install tests |

---

## Appendix B: Acceptance criteria checklist

- [ ] `sjvm install 21` downloads, verifies (SHA-256), extracts, and installs Temurin 21 on Linux x64 without errors.
- [ ] `sjvm install 21` a second time (no `--force`) exits with code 1 and the "already installed" warning, without re-downloading.
- [ ] `sjvm install 21 --force` succeeds and overwrites the existing installation.
- [ ] `sjvm install 17 --vendor graalvm` installs GraalVM CE 17.
- [ ] `sjvm install 99` exits with code 1 and the version-out-of-range error. No network call is made.
- [ ] `sjvm install 21 --vendor zulu` is rejected by clap before any network call.
- [ ] After `sjvm install 21`, running `sjvm list` shows the newly installed JDK without requiring `sjvm setup`.
- [ ] After `sjvm install 21` with the post-install `y` response, `sjvm list` shows `→ temurin-21-...` as the current JDK.
- [ ] In the TUI (`sjvm ui`), pressing `i` or `Tab` shows the Install screen with a vendor picker.
- [ ] Selecting a vendor and version in the TUI shows an inline progress bar during download.
- [ ] After a successful TUI install, a `✅ Installed` message and a switch prompt are shown.
- [ ] After a failed TUI install, a `❌ Error` message is shown and the screen returns to `Idle` on any key press.
- [ ] `cargo test --all-features` passes with zero failures.
- [ ] `cargo clippy --all-features -- -D warnings` produces zero warnings.
- [ ] `cargo audit` produces no new advisories.
- [ ] The binary contains no hardcoded API keys, tokens, or secrets.
- [ ] A `.tar.gz` archive containing a `../` path traversal entry is rejected with an error during extraction.

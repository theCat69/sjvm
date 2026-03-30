# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

## [2.0.1] - 2026-03-30

### Added

- `sjvm install <VERSION> [--vendor openjdk|graalvm] [--os OS] [--arch ARCH] [--force]` — downloads, SHA-256-verifies, extracts, and installs a JDK from Adoptium Temurin or GraalVM CE. Supported major versions: 8–25. After installation, prompts to switch immediately.
- `sjvm delete <NAME>` — removes an installed JDK directory after a `[y/N]` confirmation prompt. Validates the name against path-traversal and other illegal inputs.
- `sjvm versions [--vendor openjdk|graalvm]` — queries vendor APIs and prints available JDK major versions. Shows both vendors when `--vendor` is omitted.
- TUI install screen (`sjvm ui`, `--features ui`): a new **Install** tab reachable via `Tab` or `i`. Opens directly at the vendor picker; press `Enter` to confirm a vendor and load the version list (8–25) fetched from the API. Selecting a version starts an inline download with a human-readable progress gauge (`x.x MB / x.x MB`). After success, press `y` to switch immediately.
- TUI: `Ctrl+C` navigates back one step throughout the install flow.
- TUI switch screen: `d` key opens a delete-confirmation overlay for the selected JDK.
- `src/core/jdk_catalog.rs` — vendor API integration (Adoptium `/v3/assets/latest` and GraalVM CE GitHub Releases).
- `src/core/downloader.rs` — streaming download, SHA-256 verification, `.tar.gz`/`.zip` extraction, and installation pipeline.
- `src/infra/http.rs` — `reqwest` blocking client wrapper with `rustls-tls`, custom `User-Agent`, and per-operation timeouts.
- `infra/memory.rs`: `invalidate_memory()` helper that deletes the binary cache so the next command re-scans installed JDKs.

### Changed

- TUI refactored from a single `src/commands/ui.rs` into a module directory (`src/commands/ui/mod.rs`, `switch_screen.rs`, `install_screen.rs`).
- TUI tab bar added at the top; screens labelled `[S]witch` and `[I]nstall`.

---

## [2.0.0] - 2026-01-01

### Added

- Initial public release.
- `sjvm setup` — first-run setup: discovers installed JDKs, points the managed symlink to the first JDK found, and builds the binary cache.
- `sjvm use <VERSION>` — switches the active JDK globally via symlink.
- `sjvm use <VERSION> --local` — prints `export JAVA_HOME=...` and `export PATH=...` for the current shell session (use with `eval`).
- `sjvm list` — lists discovered JDKs; marks the currently active one with `→`.
- `sjvm config path` — prints the path to the configuration file.
- `sjvm ui` (optional, `--features ui`) — interactive TUI switch screen using ratatui.
- Cross-platform support: Linux, macOS, Windows (Developer Mode required for symlinks).
- JSON configuration file at platform-standard location (`~/.config/sjvm/sjvm-conf.json` on Linux).

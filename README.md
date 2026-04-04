# Simple Java Version Manager (sjvm)

**sjvm** is a minimalist, cross-platform Rust-based CLI tool for managing multiple Java JDK installations using symlink indirection. Similar to tools like jenv or sdkman, but with a simpler, more focused approach.

## Motivation

A few years ago, I created a project to manage the Java version currently used on my machine and within a specific terminal prompt: [blazingly-fast-java-version-manager](https://github.com/theCat69/blazingly-fast-java-version-manager).
At the time, I was just starting to learn Rust and wanted to try a different approach than the classic symlink indirection everyone uses. It worked for most use cases, but the code was messy and overly complicated. I didn't improve it much after that.

However, I kept using that tool over the years and wanted to build something better for myself.

This project aims to be a minimalist, simple, and cross-platform Java version manager using symlink indirection, built with modern Rust best practices.

## Prerequisites and Permissions

To use `sjvm`, you must have permission to read, execute, and create symlinks for the JDK folders you want to manage.

### Windows

On Windows, you need to have Developer Mode enabled: [enable-your-device-for-development](https://learn.microsoft.com/fr-fr/windows/apps/get-started/enable-your-device-for-development)

Default JDK folder:

```batch
C:\Java
```

### Linux

On Linux, if you install JDKs via a package manager and your user cannot create symlinks in those locations, you'll need to copy the JDKs to a folder you own.

Default JDK folder:

```sh
/usr/lib/jvm
```

### macOS

This is not tested. If you try it and it doesn't work, feel free to open an issue.

Default JDK folder:

```sh
/Library/Java/JavaVirtualMachines
```

## Installation

```bash
cargo install --path .
# With TUI support:
cargo install --path . --features ui
```

## Configuration

`sjvm` uses JSON for its configuration.
A simple example:

```json
{
  "jdks_dirs": [
    "C:\\dev\\compilers\\java"
  ]
}
```

You can also specify the folder `sjvm` will use as the main symlink destination:

```json
{
  "symlink_dir": "C:\\dev\\sjvm\\java"
}
```

The configuration folder depends on your system. To find the path `sjvm` uses, run:

```sh
sjvm config path
```

## Setup

Performs first-run setup: discovers installed JDKs, points the managed symlink to the first JDK found, and builds the binary cache. Run this once after installing sjvm, and again whenever you add or remove a JDK.

```sh
sjvm setup
```

## Commands

### List

List Java installations managed by `sjvm` on your device:

```sh
sjvm list
```

Example output:

```
  C:\dev\compilers\Java\jdk-17.0.1
→ C:\dev\compilers\Java\jdk-20.0.1
  C:\dev\compilers\Java\jdk-21.0.1
```

> The `→` marker indicates the currently active JDK (the one the managed symlink currently points to).

### Use

To change the Java installation for your user:

```sh
sjvm use jdk-21
```

Example output:

```
✅ Now using JDK: C:\dev\compilers\Java\jdk-21.0.1
```

`sjvm` will match the name of the folder resolved by the list command.
It will use the first match, so name your folders accordingly.

### Local Mode

Set Java version for the current shell session only. The `--local` flag prints `export JAVA_HOME=...` and `export PATH=...` to stdout — it does **not** create any file. You must `eval` the output to apply it to your current shell session:

```sh
eval $(sjvm use jdk-17 --local)
```

> **Note:** Use `eval $(sjvm use <version> --local)` to apply the environment variables to your current shell session. Without `eval`, the variables are printed but not set.

> ⚠️ **Windows Note**: Local mode is not yet supported on Windows. The command will show the manual steps to set the JAVA_HOME for the current session.

### Config

Show configuration path:

```sh
sjvm config path
```

### Install

Download, verify, and install a JDK directly from Adoptium (Temurin) or GraalVM CE:

```sh
# Install the latest Temurin JDK 21 for the current platform
sjvm install 21

# Install GraalVM CE 17
sjvm install 17 --vendor graalvm

# Overwrite an existing installation
sjvm install 21 --force
```

After a successful install, sjvm prompts you to switch to the new JDK immediately:

```
✅ Installed temurin-21-amd64 → /usr/lib/jvm/temurin-21-amd64

Switch to the newly installed JDK now? [y/N]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `--vendor <VENDOR>` | `openjdk` (Adoptium Temurin) or `graalvm` | `openjdk` |
| `--os <OS>` | Target OS override (`linux`, `mac`, `windows`) | auto-detected |
| `--arch <ARCH>` | Target architecture override (`x64`, `aarch64`) | auto-detected |
| `--force` | Re-download and overwrite if already installed | off |

Supported major versions: **8–25**.

### Delete

Remove an installed JDK directory (prompts for confirmation):

```sh
sjvm delete jdk-21
```

Example output:

```
Are you sure you want to delete "jdk-21"? [y/N] y
✓ Deleted jdk-21
```

The name must be a plain directory name — no path separators or `..` components. `sjvm delete` searches **all** configured `jdks_dirs` directories for the named JDK.

### Versions

List available JDK versions from vendor APIs (requires a network connection):

```sh
# Show versions from all vendors
sjvm versions

# Show only OpenJDK (Adoptium) versions
sjvm versions --vendor openjdk

# Show only GraalVM CE versions
sjvm versions --vendor graalvm
```

Example output:

```
OpenJDK (Adoptium): 8, 11, 17, 21, 22, 23
GraalVM CE: 17, 20, 21
```

### UI (Interactive Mode)

`sjvm` includes an optional interactive TUI for browsing and switching JDKs, and for installing new ones:

```sh
sjvm ui
```

> **Note:** The `ui` subcommand requires building with `--features ui` (see Feature Flags below).

The TUI has two screens navigable via the **Tab** key (or `s`/`i` shortcuts):

**Switch screen** — browse and activate installed JDKs:

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` | Switch to the selected JDK |
| `d` | Delete the selected JDK (confirmation overlay) |
| `q` / `Esc` | Quit |

**Install screen** — pick a vendor and version, then download and install inline:

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `Enter` | Confirm selection / start download |
| `Ctrl+C` | Go back one step |

The install screen opens directly at the **vendor picker** (OpenJDK or GraalVM CE). After selecting a vendor, sjvm fetches the available versions (8–25) from the vendor API and presents them as a scrollable list. Once a version is confirmed, download progress is shown inline as a human-readable gauge (`50.0 MB / 195.3 MB`). On success, press `y` to switch immediately or `n` / `Ctrl+C` to return.

**Global TUI keybindings (any screen):**

| Key | Action |
|-----|--------|
| `Tab` | Toggle between Switch and Install screens |
| `s` | Jump to Switch screen |
| `i` | Jump to Install screen |
| `q` / `Esc` | Quit |

## Development

### Building

```bash
# Local default toolchain comes from rust-toolchain.toml (stable)

# Build the project
cargo build

# Build for release
cargo build --release

# Run the CLI
./target/release/sjvm --help
```

### Feature Flags

| Feature | Enables | Default |
|---------|---------|---------|
| `ui` | ratatui + crossterm (interactive TUI) | ❌ off |

```bash
# Build with TUI support
cargo build --features ui

# Release build with TUI
cargo build --release --features ui

# Minimal build (no TUI)
cargo build --no-default-features
```

### Testing

```bash
# Run unit tests
cargo test

# Main local validation path
cargo fmt --check
cargo check --all-features
cargo test --all-features
cargo clippy --all-features -- -D warnings

# Run specific test
cargo test test_name

# Run minimal feature set explicitly
cargo test --no-default-features
```

### End-to-End Testing with Docker

The project includes comprehensive e2e tests that run in a Docker environment with multiple Java versions (11, 17, 21) pre-installed:

```bash
# Run e2e tests (Docker image builds automatically)
docker compose -f ./docker/it-ubuntu-compose.yaml up

# Run in detached mode
docker compose -f ./docker/it-ubuntu-compose.yaml up -d

# Force rebuild of the Docker image
docker compose -f ./docker/it-ubuntu-compose.yaml up --build

# View logs
docker compose -f ./docker/it-ubuntu-compose.yaml logs -f

# Stop containers
docker compose -f ./docker/it-ubuntu-compose.yaml down
```

The Docker setup includes:
- Ubuntu 22.04 with Java 11, 17, and 21 installed
- Rust toolchain for building and testing
- Volume mounts for live code updates during development

## Architecture

### Core Components

- **CLI Interface** (`main.rs`) — Command-line parsing and routing using clap
- **Configuration** (`infra/config.rs`) — JSON-based configuration management with cross-platform directories
- **JDK Resolution** (`core/jdk_resolver.rs`) — Automatic JDK discovery and version detection
- **JDK Switching** (`core/jdk_switcher.rs`) — Version matching and symlink switch operations
- **JDK Catalog** (`core/jdk_catalog.rs`) — Vendor API integration (Adoptium and GraalVM CE); resolves download URLs and checksums
- **Downloader** (`core/downloader.rs`) — Streaming download, SHA-256 verification, archive extraction, and installation pipeline
- **HTTP Client** (`infra/http.rs`) — `reqwest` blocking client with `rustls-tls`; centralises TLS config and `User-Agent`
- **Symlink Management** (`infra/symlinks.rs`) — Cross-platform symlink operations
- **Memory Management** (`infra/memory.rs`) — Binary cache (bincode) storing current JDK state
- **Commands** — Individual CLI command implementations:
  - `commands/setup.rs` — First-run setup
  - `commands/use_cmd.rs` — Switch JDK globally or locally
  - `commands/list.rs` — List installed JDKs
  - `commands/install.rs` — Download and install a JDK
  - `commands/delete.rs` — Remove an installed JDK
  - `commands/versions.rs` — List available versions from vendor APIs
  - `commands/ui/` — Interactive TUI (feature-gated): `mod.rs` (screen enum, event loop), `switch_screen.rs`, `install_screen.rs`

### Configuration

Configuration is stored in platform-specific directories using the `directories` crate:

- **Linux**: `~/.config/sjvm/sjvm-conf.json`
- **macOS**: `~/Library/Application Support/sjvm/sjvm-conf.json`
- **Windows**: `%APPDATA%\sjvm\sjvm-conf.json`

Example configuration:
```json
{
  "jdks_dirs": [
    "C:\\dev\\compilers\\java",
    "/usr/lib/jvm",
    "/Library/Java/JavaVirtualMachines"
  ],
  "symlink_dir": "C:\\dev\\sjvm\\java"
}
```

## Requirements

- Rust 1.88+ (Edition 2024; local default toolchain is stable via `rust-toolchain.toml`)
- Permission to create symlinks in JDK directories
- Windows: Developer Mode enabled for symlink creation
- Docker (for e2e testing)

## License

MIT — see [LICENSE](LICENSE) for details.

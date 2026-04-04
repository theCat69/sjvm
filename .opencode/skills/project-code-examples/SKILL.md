---
name: project-code-examples
description: Catalog of project code examples — what patterns exist and where to find them in .code-examples-for-ai/
---

# Project Code Examples

These examples demonstrate the coding patterns used in this project. Each file contains a single representative snippet extracted from the real production source, annotated to explain what to imitate.

## Available Examples

| File | Pattern |
|------|---------|
| `clap-subcommand.md` | Clap derive API: `Parser`/`Subcommand` structs, `value_parser`, `ValueEnum`, feature-gated subcommands, `try_parse_from` in tests |
| `anyhow-error-handling.md` | anyhow error handling: `with_context`, `bail!`, `ensure!`, `?` propagation, never-attach-secrets rule |
| `oncelock-singleton.md` | Lazy-initialized globals: `OnceLock<T>` (immutable), `LazyLock<Mutex<Option<T>>>` (invalidatable), scoped-static client pattern |
| `pure-parse-http-split.md` | Pure function / HTTP-calling function separation: all JSON parsing logic is a pure function testable with fixtures; HTTP wrapper calls the pure function |
| `config-oncelock-singleton.md` | Config singleton with partial JSON merge, `validate_no_traversal` security guard, platform-default path resolution via `directories` crate |
| `bincode-cache.md` | bincode 2.0 `Encode`/`Decode` derive; `encode_to_vec` / `decode_from_slice`; round-trip test; corrupted-bytes rejection |
| `unit-test-patterns.md` | `#[cfg(test)] mod tests`, `use super::*`, `anyhow::Result<()>` return type, parametric test loops, error-message assertions, pure-function fixture pattern |
| `platform-symlinks.md` | `#[cfg(unix)]` / `#[cfg(target_os = "windows")]` symlink guards; atomic replace pattern; cross-device rename → copy fallback |
| `https-only-http-client.md` | `require_https()` guard; `HeaderValue::set_sensitive(true)` for `GITHUB_TOKEN`; `use_rustls_tls()` + `danger_accept_invalid_certs(false)`; OnceLock HTTP client singleton |

## Location

`.code-examples-for-ai/`

## Maintenance

This index is maintained by the AI. Developers may add entries manually. One file per pattern.

When a feature introduces a new coding pattern not already represented here:
1. Create a new `.md` file in `.code-examples-for-ai/` named after the pattern in kebab-case.
2. Include a one-line description comment at the top.
3. Add a real snippet from production code with brief inline annotations.
4. Add the new entry to the `## Available Examples` table above.

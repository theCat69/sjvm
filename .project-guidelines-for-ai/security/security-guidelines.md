# Security Guidelines

Security best practices for **sjvm** — a Rust CLI tool for managing Java JDK symlinks.

---

## Secrets Management

- **Never hardcode secrets** (API keys, tokens, passwords) in source code or configuration files.
- Use environment variables for any credentials: `std::env::var("SOME_KEY")`.
- Never commit `.env` files — add them to `.gitignore`.
- Config loading order: defaults → config file → environment variables (environment wins).
- **Critical — anyhow context strings**: do not attach secret values (tokens, passwords) as `.context("...")` strings. They appear in error logs and terminal output.
  ```rust
  // WRONG — token visible in error output
  fetch(url).with_context(|| format!("request failed, token={}", token))?;

  // CORRECT — context without secrets
  fetch(url).context("request to registry failed")?;
  ```
- **CLI argument security**: never accept secrets via positional arguments — they are visible in process listings (`ps aux`). Use environment variables or interactive prompts instead.
  ```rust
  // Prefer env fallback over positional secret
  #[arg(env = "SJVM_TOKEN", hide_env_values = true)]
  token: Option<String>,
  ```

---

## Input Validation

### Path arguments

User-supplied paths must be validated against path traversal attacks:

- Reject paths containing `..` components.
- Use `Path::canonicalize()` and verify the result is a prefix of an expected directory.
- In clap, use a custom `value_parser` to validate at parse time:
  ```rust
  #[arg(value_parser = validate_no_traversal)]
  path: PathBuf,

  fn validate_no_traversal(s: &str) -> Result<PathBuf, String> {
      let p = PathBuf::from(s);
      if p.components().any(|c| c == std::path::Component::ParentDir) {
          return Err("path must not contain '..'".to_string());
      }
      Ok(p)
  }
  ```

### Version strings

- Validate that version arguments match expected patterns (numeric, no shell metacharacters).
- Reject empty strings and overly long inputs.
- Use `value_enum` with `#[derive(ValueEnum)]` for closed sets of valid values — clap rejects unknown values automatically.

### Integer safety

- In debug builds, integer overflow panics by default — this is intentional for catching bugs.
- In release builds, integers wrap silently. Use `checked_add`, `saturating_add`, or `overflowing_add` explicitly when overflow is a concern.
- Use `array.get(index)` returning `Option` instead of `array[index]` for user-controlled indices.

---

## Dependency Security

Run these tools regularly and in CI:

### `cargo audit`

Checks all dependencies against the [RustSec Advisory Database](https://rustsec.org/):

```bash
cargo install cargo-audit
cargo audit
```

- Run in CI and fail the build on any advisory.
- Advisories may be unmaintained crates, vulnerabilities, or unsound APIs.

### `cargo deny`

Policy-based supply chain control — configure in `deny.toml`:

```bash
cargo install cargo-deny
cargo deny check
```

`deny.toml` can enforce:
- Banned crates (known problematic dependencies).
- License policy (e.g. only MIT/Apache-2.0 allowed).
- Duplicate detection (same crate at multiple versions).
- Advisory database (same as `cargo audit` but integrated into deny).

### `cargo geiger`

Audits unsafe usage in the dependency tree:

```bash
cargo install cargo-geiger
cargo geiger
```

Use this to understand how much unsafe code is introduced by dependencies.

### `cargo outdated`

Detects stale dependencies:

```bash
cargo install cargo-outdated
cargo outdated
```

### General dependency hygiene

- Prefer well-maintained crates with high download counts and recent commits.
- Review crate source before adding (crates.io links to docs.rs and GitHub).
- Use `default-features = false` and enable only the features you need — reduces attack surface.
- Do not add heavy dependencies without justification.
- **Cargo.lock must be committed** for this binary — it pins exact versions for reproducible and auditable builds.

---

## Authentication & Authorization

sjvm is a local CLI tool with no network authentication requirements in its current form. If authentication is added in the future:

- Use OS-level credential stores (e.g. `keyring` crate) rather than plaintext files.
- Validate that config files have appropriate permissions (not world-readable if they contain sensitive paths).
- Never log authentication tokens, even at debug level.

---

## Common Vulnerabilities

### Symlink attacks (TOCTOU)

sjvm manages symlinks, which introduces time-of-check / time-of-use (TOCTOU) risks:

- After resolving a JDK path, verify the target still exists before creating the symlink.
- Do not follow symlinks through user-controlled directories to sensitive paths.
- On Unix, consider using `O_NOFOLLOW` when opening files through paths that involve symlinks.

### Unsafe code

- Mark every `unsafe` block with a `// SAFETY: <invariant>` comment explaining why it is sound.
- Prefer safe alternatives — most standard library operations do not require `unsafe`.
- In Rust 2024 edition, `unsafe_op_in_unsafe_fn` is deny-by-default: calls inside an `unsafe fn` still require their own `unsafe` block with a `SAFETY` comment.
- Consider `#![deny(unsafe_code)]` if the binary has no legitimate need for unsafe.

### Memory safety (Rust guarantees at compile time)

Rust prevents at compile time:
- Use-after-free
- Double-free
- Buffer overflow
- Null pointer dereference
- Data races

These are not runtime concerns for safe Rust code. The main security attack surface for sjvm is **path manipulation** and **input validation**, not memory safety.

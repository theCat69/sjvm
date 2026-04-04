<!-- Demonstrates: HTTPS-only enforcement guard; OnceLock HTTP client singletons; sensitive header handling; streaming download with 2 GiB cap -->

```rust
//! HTTP utilities for sjvm — all I/O is HTTPS-only, TLS via rustls.

use std::{env, sync::OnceLock, time::Duration};
use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;

// ---------------------------------------------------------------------------
// HTTPS enforcement guard — called before every outbound request
// ---------------------------------------------------------------------------

fn require_https(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        // Reject non-HTTPS URLs before making any network call
        bail!("Refusing to download over non-HTTPS URL: {url}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sensitive header handling — GITHUB_TOKEN never stored as a plain string
// ---------------------------------------------------------------------------

fn build_client_builder() -> Result<ClientBuilder> {
    let user_agent = format!("sjvm/{} (https://github.com/fefou/sjvm)", env!("CARGO_PKG_VERSION"));

    let mut default_headers = HeaderMap::new();
    let ua_value = HeaderValue::from_str(&user_agent)
        .context("Failed to construct User-Agent header value")?;
    default_headers.insert(USER_AGENT, ua_value);

    if let Ok(token) = env::var("GITHUB_TOKEN") {
        let bearer = format!("Bearer {token}");
        let mut auth_value = HeaderValue::from_str(&bearer)
            .context("GITHUB_TOKEN contains characters invalid for an HTTP header value")?;
        // set_sensitive(true): header value is redacted in logs/debug output
        auth_value.set_sensitive(true);
        default_headers.insert(AUTHORIZATION, auth_value);
        // `token` and `bearer` are dropped here — never stored beyond this scope
    }

    Ok(Client::builder()
        .use_rustls_tls()                   // always rustls; never native-tls
        .danger_accept_invalid_certs(false) // never disable TLS verification
        .connect_timeout(Duration::from_secs(10))
        .default_headers(default_headers))
}

// ---------------------------------------------------------------------------
// OnceLock HTTP client singleton (scoped static inside function)
// ---------------------------------------------------------------------------

fn short_client() -> Result<&'static Client> {
    static SHORT_CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(c) = SHORT_CLIENT.get() {
        return Ok(c);
    }
    let client = build_client_builder()?
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to build short-timeout HTTP client")?;
    Ok(SHORT_CLIENT.get_or_init(|| client))
}

// ---------------------------------------------------------------------------
// Public API — every function calls require_https before sending
// ---------------------------------------------------------------------------

pub(crate) fn get_json(url: &str) -> Result<Value> {
    require_https(url)?;                          // ← always first
    let response = short_client()?.get(url).send()
        .with_context(|| format!("HTTP GET request failed for {url}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} from {url}");
    }
    response.json::<Value>()
        .with_context(|| format!("Failed to deserialise JSON response from {url}"))
}

// ---------------------------------------------------------------------------
// 2 GiB download cap — prevents archive bomb attacks in the streaming loop
// ---------------------------------------------------------------------------

// Maximum allowed response body size. Enforced in the read loop;
// the partial destination file is deleted before bailing.
const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB

// Illustrative excerpt — actual loop inside download_streaming():
//
//     bytes_downloaded += n as u64;
//     if bytes_downloaded > MAX_DOWNLOAD_BYTES {
//         drop(writer);
//         let _ = std::fs::remove_file(dest);
//         bail!("Download aborted: response exceeded maximum allowed size of 2 GiB");
//     }

// --- Tests ------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // Test that require_https is enforced — no network call should happen
    #[test]
    fn test_rejects_http_url_get_json() {
        let result = get_json("http://example.com");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("non-HTTPS"), "error should mention 'non-HTTPS', got: {msg}");
    }
}
```

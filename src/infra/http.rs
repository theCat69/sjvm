//! HTTP utilities for sjvm — all I/O is HTTPS-only, TLS via rustls.

use std::{
    env,
    fs::File,
    io::{BufWriter, Read as _, Write as _},
    path::Path,
    sync::OnceLock,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Client configuration
// ---------------------------------------------------------------------------

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SHORT_TIMEOUT: Duration = Duration::from_secs(30);
const LONG_TIMEOUT: Duration = Duration::from_secs(600);

/// Maximum number of bytes allowed in a single download (2 GiB).
const MAX_DOWNLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Builds a [`ClientBuilder`] pre-configured with the shared defaults:
/// rustls TLS, User-Agent header, and — when `GITHUB_TOKEN` is set at
/// runtime — an `Authorization: Bearer` default header.
///
/// The token is consumed directly into the header value and is **never**
/// stored as a plain string after this function returns.
fn build_client_builder() -> Result<ClientBuilder> {
    let user_agent = format!(
        "sjvm/{} (https://github.com/fefou/sjvm)",
        env!("CARGO_PKG_VERSION")
    );

    let mut default_headers = HeaderMap::new();

    let ua_value = HeaderValue::from_str(&user_agent)
        .context("Failed to construct User-Agent header value")?;
    default_headers.insert(USER_AGENT, ua_value);

    if let Ok(token) = env::var("GITHUB_TOKEN") {
        // Construct "Bearer <token>" without ever formatting it into a wider string.
        let bearer = format!("Bearer {token}");
        let mut auth_value = HeaderValue::from_str(&bearer)
            .context("GITHUB_TOKEN contains characters invalid for an HTTP header value")?;
        auth_value.set_sensitive(true);
        default_headers.insert(AUTHORIZATION, auth_value);
    }

    Ok(Client::builder()
        .use_rustls_tls()
        .danger_accept_invalid_certs(false)
        .connect_timeout(CONNECT_TIMEOUT)
        .default_headers(default_headers))
}

fn short_client() -> Result<&'static Client> {
    static SHORT_CLIENT: OnceLock<Client> = OnceLock::new();
    if SHORT_CLIENT.get().is_none() {
        let client = build_client_builder()
            .context("Failed to configure HTTP client")?
            .timeout(SHORT_TIMEOUT)
            .build()
            .context("Failed to build short-timeout HTTP client")?;
        // OnceLock::set can fail if another thread races, but the value it
        // already contains is just as valid — return whichever wins.
        let _ = SHORT_CLIENT.set(client);
    }
    SHORT_CLIENT
        .get()
        .context("BUG: short HTTP client not initialized after init")
}

fn long_client() -> Result<&'static Client> {
    static LONG_CLIENT: OnceLock<Client> = OnceLock::new();
    if LONG_CLIENT.get().is_none() {
        let client = build_client_builder()
            .context("Failed to configure HTTP client")?
            .timeout(LONG_TIMEOUT)
            .build()
            .context("Failed to build long-timeout HTTP client")?;
        // OnceLock::set can fail if another thread races, but the value it
        // already contains is just as valid — return whichever wins.
        let _ = LONG_CLIENT.set(client);
    }
    LONG_CLIENT
        .get()
        .context("BUG: long HTTP client not initialized after init")
}

// ---------------------------------------------------------------------------
// HTTPS enforcement
// ---------------------------------------------------------------------------

fn require_https(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        bail!("Refusing to download over non-HTTPS URL: {url}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Issues a GET request to `url` and deserialises the response body as JSON.
///
/// Uses a 30-second total timeout. Requires an `https://` URL.
pub(crate) fn get_json(url: &str) -> Result<Value> {
    require_https(url)?;

    let response = short_client()?
        .get(url)
        .send()
        .with_context(|| format!("HTTP GET request failed for {url}"))?;

    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} from {url}");
    }

    response
        .json::<Value>()
        .with_context(|| format!("Failed to deserialise JSON response from {url}"))
}

/// Issues a GET request to `url` and returns the response body as a UTF-8 string.
///
/// Uses a 30-second total timeout. Requires an `https://` URL.
pub(crate) fn get_text(url: &str) -> Result<String> {
    require_https(url)?;

    let response = short_client()?
        .get(url)
        .send()
        .with_context(|| format!("HTTP GET request failed for {url}"))?;

    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} from {url}");
    }

    response
        .text()
        .with_context(|| format!("Failed to read response body as text from {url}"))
}

/// Downloads `url` to `dest`, calling `on_progress(bytes_downloaded, total)` after each chunk.
///
/// `total` is `Some(n)` when the server supplies a `Content-Length` header;
/// otherwise `None`.  Uses a 600-second total timeout. Requires an `https://` URL.
/// The output file is written via [`BufWriter`] for efficiency.
pub(crate) fn download_streaming(
    url: &str,
    dest: &Path,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<()> {
    require_https(url)?;

    let response = long_client()?
        .get(url)
        .send()
        .with_context(|| format!("HTTP GET request failed for {url}"))?;

    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} from {url}");
    }

    let content_length: Option<u64> = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    let file = File::create(dest)
        .with_context(|| format!("Failed to create destination file: {}", dest.display()))?;
    let mut writer = BufWriter::new(file);

    let mut reader = response;
    let mut bytes_downloaded: u64 = 0;

    // Read the response body in 64 KiB chunks, tracking progress.
    let mut buf = vec![0u8; 65_536];
    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("Failed to read response body from {url}"))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .with_context(|| format!("Failed to write to {}", dest.display()))?;
        bytes_downloaded += n as u64;
        if bytes_downloaded > MAX_DOWNLOAD_BYTES {
            drop(writer);
            let _ = std::fs::remove_file(dest);
            bail!("Download aborted: response exceeded maximum allowed size of 2 GiB from {url}");
        }
        on_progress(bytes_downloaded, content_length);
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush file: {}", dest.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// `get_json` must reject plain-HTTP URLs before making any network request.
    #[test]
    fn test_rejects_http_url_get_json() {
        let result = get_json("http://example.com");
        assert!(result.is_err(), "expected Err for non-HTTPS URL");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("non-HTTPS"),
            "error message should mention 'non-HTTPS', got: {msg}"
        );
    }

    /// `get_text` must reject plain-HTTP URLs before making any network request.
    #[test]
    fn test_rejects_http_url_get_text() {
        let result = get_text("http://example.com");
        assert!(result.is_err(), "expected Err for non-HTTPS URL");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("non-HTTPS"),
            "error message should mention 'non-HTTPS', got: {msg}"
        );
    }

    /// `download_streaming` must reject plain-HTTP URLs before making any network request.
    #[test]
    fn test_rejects_http_url_download() {
        let path = PathBuf::from("/tmp/sjvm_test_should_not_exist");
        let result = download_streaming("http://example.com", &path, |_, _| {});
        assert!(result.is_err(), "expected Err for non-HTTPS URL");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("non-HTTPS"),
            "error message should mention 'non-HTTPS', got: {msg}"
        );
    }
}

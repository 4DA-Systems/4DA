// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Toolkit — HTTP Probe subsystem.
//!
//! Provides HTTP probing and history tracking for the Developer Toolkit.
//! Extracted from `toolkit.rs` to keep file sizes manageable.

use crate::error::{FourDaError, Result};
use crate::state::get_database;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHistoryEntry {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub created_at: String,
}

// ============================================================================
// HTTP Probe
// ============================================================================

/// Allowlist of remote domains that toolkit HTTP requests may target.
/// Covers LLM providers, source APIs and license validation. Local hosts are
/// handled separately by [`LOCAL_HOSTS`] and are pinned to the Ollama port.
const ALLOWED_DOMAINS: &[&str] = &[
    // LLM providers
    "api.openai.com",
    "api.anthropic.com",
    "generativelanguage.googleapis.com",
    // License validation
    "api.keygen.sh",
    // Source APIs
    "hacker-news.firebaseio.com",
    "www.reddit.com",
    "oauth.reddit.com",
    "api.github.com",
    "api.x.com",
    "export.arxiv.org",
    "www.youtube.com",
    "lobste.rs",
    "dev.to",
    "www.producthunt.com",
];

/// Local hosts the probe may reach — only on [`OLLAMA_PORT`]. Any other local
/// port (Victauri's MCP bridge, the Signal Terminal, dev servers, a user's own
/// services) is out of bounds: the probe is a renderer-reachable command, so an
/// open localhost allowance would turn it into a bridge to every local service.
const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0"];

/// The only local port the probe may target (Ollama's default).
const OLLAMA_PORT: u16 = 11434;

/// Redirect hops the probe follows before giving up.
const MAX_PROBE_REDIRECTS: usize = 5;

/// Decide whether a parsed URL is a permitted probe target.
///
/// The host comes from a real URL parser (`url::Url`), never from string
/// splitting: `https://api.openai.com:443@attacker.example/` has host
/// `attacker.example`, which a hand-split parser read as `api.openai.com`.
/// Userinfo is rejected outright, and the host must match exactly — a
/// trailing-dot FQDN (`api.openai.com.`) is not silently normalised in.
fn is_parsed_url_allowed(url: &url::Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if LOCAL_HOSTS.iter().any(|h| host.eq_ignore_ascii_case(h)) {
        return url.port_or_known_default() == Some(OLLAMA_PORT);
    }
    ALLOWED_DOMAINS
        .iter()
        .any(|allowed| host.eq_ignore_ascii_case(allowed))
}

/// Parse and check a probe URL string.
fn is_url_allowed(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| is_parsed_url_allowed(&parsed))
}

/// Client for the toolkit probe: every redirect hop is re-checked against the
/// same allowlist, so an allowed host cannot bounce the request elsewhere.
static TOOLKIT_CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .timeout(std::time::Duration::from_secs(30))
        .connect_timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_PROBE_REDIRECTS {
                attempt.error(format!("too many redirects (limit {MAX_PROBE_REDIRECTS})"))
            } else if is_parsed_url_allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("redirect blocked: hop leaves the toolkit allowlist")
            }
        }))
        .build()
        .unwrap_or_else(|e| {
            warn!(target: "4da::toolkit", error = %e, "Failed to build toolkit client; redirects disabled");
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default()
        })
});

#[tauri::command]
pub async fn toolkit_http_request(request: HttpProbeRequest) -> Result<HttpProbeResponse> {
    // Validate URL
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err(FourDaError::Config(
            "URL must start with http:// or https://".into(),
        ));
    }

    // Enforce domain allowlist to prevent data exfiltration
    if !is_url_allowed(&request.url) {
        return Err(FourDaError::Config(format!(
            "Domain not allowed. Requests are restricted to known APIs (LLM providers, source APIs) and local Ollama (port {OLLAMA_PORT}); URLs with embedded credentials are rejected."
        )));
    }

    let method = request
        .method
        .parse::<reqwest::Method>()
        .map_err(|e| FourDaError::Config(format!("Invalid HTTP method: {e}")))?;

    let mut req = TOOLKIT_CLIENT.request(method, &request.url);

    for (key, value) in &request.headers {
        req = req.header(key.as_str(), value.as_str());
    }

    if let Some(body) = &request.body {
        req = req.body(body.clone());
    }

    let start = Instant::now();
    let response = req
        .send()
        .await
        .map_err(|e| FourDaError::Internal(format!("HTTP request failed: {e}")))?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();

    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<binary or unreadable>".into());
    let size_bytes = body.len();

    // Truncate very large responses. Snap to a char boundary — the URL is
    // user-supplied, so a raw byte cut panics this command on any large
    // non-ASCII response body.
    let body = if body.len() > 500_000 {
        format!(
            "{}...\n\n(truncated at 500KB)",
            &body[..body.floor_char_boundary(500_000)]
        )
    } else {
        body
    };

    // Save to history (non-fatal)
    if let Err(e) = save_http_history(&request.method, &request.url, status, duration_ms) {
        warn!(target: "4da::toolkit", error = %e, "Failed to save HTTP history");
    }

    debug!(target: "4da::toolkit", url = %request.url, status, duration_ms, "HTTP probe complete");

    Ok(HttpProbeResponse {
        status,
        status_text,
        headers,
        body,
        duration_ms,
        size_bytes,
    })
}

#[tauri::command]
pub async fn toolkit_get_http_history(limit: Option<u32>) -> Result<Vec<HttpHistoryEntry>> {
    let limit = limit.unwrap_or(50).min(200);
    let db = get_database()?;

    let rows = db.get_http_history(limit).map_err(FourDaError::Db)?;

    Ok(rows
        .into_iter()
        .map(|r| HttpHistoryEntry {
            id: r.id,
            method: r.method,
            url: r.url,
            status: r.status,
            duration_ms: r.duration_ms,
            created_at: r.created_at,
        })
        .collect())
}

/// Persist an HTTP request in the history table.
fn save_http_history(method: &str, url: &str, status: u16, duration_ms: u64) -> Result<()> {
    let db = get_database()?;
    db.save_http_history(method, url, status, duration_ms)
        .map_err(FourDaError::Db)?;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Struct construction & serde roundtrip ---------------------------------

    #[test]
    fn http_probe_request_serde_roundtrip() {
        let req = HttpProbeRequest {
            method: "POST".into(),
            url: "https://example.com/api".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Some("{\"key\":\"value\"}".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let restored: HttpProbeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.method, "POST");
        assert_eq!(restored.url, "https://example.com/api");
        assert_eq!(restored.headers.len(), 1);
        assert_eq!(restored.body.as_deref(), Some("{\"key\":\"value\"}"));
    }

    #[test]
    fn http_probe_response_serde_roundtrip() {
        let resp = HttpProbeResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![("content-type".into(), "text/html".into())],
            body: "<html></html>".into(),
            duration_ms: 42,
            size_bytes: 13,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: HttpProbeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.status, 200);
        assert_eq!(restored.status_text, "OK");
        assert_eq!(restored.duration_ms, 42);
        assert_eq!(restored.size_bytes, 13);
    }

    #[test]
    fn http_history_entry_serde_roundtrip() {
        let entry = HttpHistoryEntry {
            id: 1,
            method: "GET".into(),
            url: "https://example.com".into(),
            status: 404,
            duration_ms: 150,
            created_at: "2025-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: HttpHistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, 1);
        assert_eq!(restored.status, 404);
        assert_eq!(restored.created_at, "2025-01-01T00:00:00Z");
    }

    // -- URL validation logic (extracted from toolkit_http_request) ------------

    #[test]
    fn url_validation_rejects_invalid_schemes() {
        let invalid_urls = ["ftp://example.com", "ws://localhost", "example.com", ""];
        for url in &invalid_urls {
            let valid = url.starts_with("http://") || url.starts_with("https://");
            assert!(!valid, "URL '{}' should be rejected", url);
        }
    }

    #[test]
    fn url_validation_accepts_valid_schemes() {
        let valid_urls = [
            "http://localhost:3000",
            "https://api.example.com/v1/data",
            "http://127.0.0.1:8080/path?q=1",
        ];
        for url in &valid_urls {
            let valid = url.starts_with("http://") || url.starts_with("https://");
            assert!(valid, "URL '{}' should be accepted", url);
        }
    }

    // -- Allowlist: parser-based host matching (audit 2026-10-07) -------------

    #[test]
    fn allowlist_rejects_userinfo_disguises() {
        for url in [
            "https://api.openai.com:443@attacker.example/",
            "https://api.openai.com@evil/",
            "https://user:pass@api.openai.com/",
            "http://api.openai.com@127.0.0.1:11434/",
        ] {
            assert!(!is_url_allowed(url), "{url} must be rejected");
        }
    }

    #[test]
    fn allowlist_ignores_host_lookalikes_in_path_and_query() {
        for url in [
            "https://evil/?x=api.openai.com",
            "https://evil/api.openai.com",
            "https://evil#api.openai.com",
            "https://api.openai.com.evil.example/",
        ] {
            assert!(!is_url_allowed(url), "{url} must be rejected");
        }
    }

    #[test]
    fn allowlist_rejects_trailing_dot_fqdn() {
        assert!(!is_url_allowed("https://API.OPENAI.COM./"));
        assert!(!is_url_allowed("https://api.openai.com./v1/models"));
    }

    #[test]
    fn allowlist_accepts_plain_allowed_hosts() {
        for url in [
            "https://api.openai.com/v1/models",
            "https://API.OPENAI.COM/v1/models",
            "https://api.anthropic.com/v1/messages",
            "https://api.github.com/repos/x/y",
            "https://hacker-news.firebaseio.com/v0/topstories.json",
            "https://api.openai.com:443/v1/models",
        ] {
            assert!(is_url_allowed(url), "{url} must be allowed");
        }
    }

    #[test]
    fn allowlist_pins_local_hosts_to_ollama_port() {
        for url in [
            "http://localhost:11434/api/tags",
            "http://127.0.0.1:11434/api/tags",
            "http://0.0.0.0:11434/",
        ] {
            assert!(is_url_allowed(url), "{url} must be allowed");
        }
        for url in [
            "http://localhost:7373/mcp",
            "http://127.0.0.1:7373/mcp",
            "http://localhost/",
            "https://localhost/",
            "http://localhost:4444/",
            "http://[::1]:11434/",
        ] {
            assert!(!is_url_allowed(url), "{url} must be rejected");
        }
    }

    #[test]
    fn allowlist_rejects_non_http_schemes_and_garbage() {
        for url in [
            "ftp://api.openai.com/",
            "file:///etc/passwd",
            "",
            "not a url",
        ] {
            assert!(!is_url_allowed(url), "{url:?} must be rejected");
        }
    }

    #[test]
    fn redirect_hops_are_checked_against_the_same_allowlist() {
        let ok = url::Url::parse("https://api.github.com/x").unwrap();
        let bad = url::Url::parse("https://attacker.example/x").unwrap();
        let local = url::Url::parse("http://127.0.0.1:7373/mcp").unwrap();
        assert!(is_parsed_url_allowed(&ok));
        assert!(!is_parsed_url_allowed(&bad));
        assert!(!is_parsed_url_allowed(&local));
    }

    // -- History limit clamping -----------------------------------------------

    #[test]
    fn history_limit_defaults_to_50() {
        let limit: Option<u32> = None;
        let clamped = limit.unwrap_or(50).min(200);
        assert_eq!(clamped, 50);
    }

    #[test]
    fn history_limit_clamps_to_200() {
        let limit: Option<u32> = Some(999);
        let clamped = limit.unwrap_or(50).min(200);
        assert_eq!(clamped, 200);
    }

    #[test]
    fn history_limit_passes_through_valid_value() {
        let limit: Option<u32> = Some(100);
        let clamped = limit.unwrap_or(50).min(200);
        assert_eq!(clamped, 100);
    }

    // -- Body truncation logic ------------------------------------------------

    #[test]
    fn body_truncation_at_500kb() {
        // Body under 500KB should not be truncated
        let small_body = "x".repeat(1000);
        let result = if small_body.len() > 500_000 {
            format!("{}...\n\n(truncated at 500KB)", &small_body[..500_000])
        } else {
            small_body.clone()
        };
        assert_eq!(result.len(), 1000);

        // Body over 500KB should be truncated
        let big_body = "y".repeat(600_000);
        let result = if big_body.len() > 500_000 {
            format!("{}...\n\n(truncated at 500KB)", &big_body[..500_000])
        } else {
            big_body.clone()
        };
        assert!(result.len() < big_body.len());
        assert!(result.contains("(truncated at 500KB)"));
        // 500_000 bytes + "...\n\n(truncated at 500KB)" suffix
        assert!(result.starts_with("yyyyy"));
    }

    // -- HttpProbeRequest with no body / no headers ---------------------------

    #[test]
    fn http_probe_request_minimal() {
        let req = HttpProbeRequest {
            method: "GET".into(),
            url: "http://localhost".into(),
            headers: vec![],
            body: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let restored: HttpProbeRequest = serde_json::from_str(&json).unwrap();
        assert!(restored.body.is_none());
        assert!(restored.headers.is_empty());
    }
}

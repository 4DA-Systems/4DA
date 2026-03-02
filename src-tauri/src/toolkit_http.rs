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

#[tauri::command]
pub async fn toolkit_http_request(request: HttpProbeRequest) -> Result<HttpProbeResponse> {
    // Validate URL
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err(FourDaError::Config(
            "URL must start with http:// or https://".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(false)
        .build()
        .map_err(|e| FourDaError::Internal(format!("Failed to build HTTP client: {e}")))?;

    let method = request
        .method
        .parse::<reqwest::Method>()
        .map_err(|e| FourDaError::Config(format!("Invalid HTTP method: {e}")))?;

    let mut req = client.request(method, &request.url);

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

    // Truncate very large responses
    let body = if body.len() > 500_000 {
        format!("{}...\n\n(truncated at 500KB)", &body[..500_000])
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

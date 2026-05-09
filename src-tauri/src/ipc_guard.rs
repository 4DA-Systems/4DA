// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! IPC input validation and rate limiting for Tauri commands.
//!
//! Provides reusable validation functions for high-risk IPC endpoints
//! (file paths, URLs, search queries, large text inputs).

use crate::error::{FourDaError, Result};

/// Maximum length for general string inputs (search queries, names, labels)
pub const MAX_INPUT_LENGTH: usize = 10_000;

/// Maximum length for content/body inputs (feedback text, descriptions)
pub const MAX_CONTENT_LENGTH: usize = 50_000;

/// Maximum length for URL inputs
pub const MAX_URL_LENGTH: usize = 2_048;

/// Maximum length for file path inputs
pub const MAX_PATH_LENGTH: usize = 1_024;

/// Validate a string input doesn't exceed the given max length.
/// Returns the trimmed input or an error.
pub(crate) fn validate_length(field: &str, value: &str, max: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.len() > max {
        tracing::warn!(
            target: "4da::ipc",
            field,
            len = trimmed.len(),
            max,
            "Input exceeds maximum length"
        );
        return Err(FourDaError::Validation(format!(
            "{field} exceeds maximum length of {max} characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// Validate a string input doesn't contain null bytes (potential injection).
pub(crate) fn validate_no_null_bytes(field: &str, value: &str) -> Result<()> {
    if value.contains('\0') {
        tracing::warn!(
            target: "4da::ipc",
            field,
            "Input contains null bytes"
        );
        return Err(FourDaError::Validation(format!(
            "{field} contains invalid characters"
        )));
    }
    Ok(())
}

/// Validate a URL input: length + no null bytes + scheme validation.
pub(crate) fn validate_url_input(field: &str, url: &str) -> Result<String> {
    let clean = validate_length(field, url, MAX_URL_LENGTH)?;
    validate_no_null_bytes(field, &clean)?;

    // Reject websocket schemes for REST relay endpoints
    let lower = clean.to_lowercase();
    if lower.starts_with("ws://") || lower.starts_with("wss://") {
        return Err(FourDaError::Validation(format!(
            "{field} must use http:// or https:// scheme, not WebSocket"
        )));
    }

    // Basic URL scheme validation
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(FourDaError::Validation(format!(
            "{field} must start with http:// or https://"
        )));
    }

    Ok(clean)
}

/// Validate a file path input: length + no null bytes + no traversal.
pub(crate) fn validate_path_input(field: &str, path: &str) -> Result<String> {
    let clean = validate_length(field, path, MAX_PATH_LENGTH)?;
    validate_no_null_bytes(field, &clean)?;
    if clean.contains("..") {
        tracing::warn!(
            target: "4da::ipc",
            field,
            "Path contains traversal sequence"
        );
        return Err(FourDaError::Validation(format!(
            "{field} contains path traversal"
        )));
    }
    Ok(clean)
}

/// Validate a file path by resolving symlinks and ensuring the canonical path
/// is safe. Use this instead of `validate_path_input` when the path will be
/// used for actual filesystem access (reads, writes, directory listing).
// REMOVE BY 2026-08-01
#[allow(dead_code)]
///
/// Performs all checks from `validate_path_input` plus:
/// - Resolves symlinks via `std::fs::canonicalize()`
/// - Blocks Windows UNC paths (`\\server\share`)
/// - Optionally validates the resolved path is under an allowed root
///
/// Returns the canonicalized path as a string.
pub(crate) fn validate_path_canonical(
    field: &str,
    path: &str,
    allowed_root: Option<&std::path::Path>,
) -> Result<String> {
    // First run the basic string-level checks
    let clean = validate_path_input(field, path)?;

    // Block Windows UNC paths (\\server\share or //server/share)
    if clean.starts_with("\\\\") || clean.starts_with("//") {
        tracing::warn!(
            target: "4da::security",
            field,
            "UNC path blocked"
        );
        return Err(FourDaError::Validation(format!(
            "{field} contains a UNC network path which is not allowed"
        )));
    }

    // Resolve symlinks and normalize the path
    let canonical = std::fs::canonicalize(&clean).map_err(|e| {
        tracing::warn!(
            target: "4da::security",
            field,
            path = %clean,
            error = %e,
            "Failed to canonicalize path"
        );
        FourDaError::Validation(format!("{field} could not be resolved to a real path: {e}"))
    })?;

    let canonical_str = canonical.to_string_lossy().to_string();

    // On Windows, canonicalize returns \\?\ extended-length paths — strip the prefix
    // for usability but keep the resolved path.
    let normalized = if cfg!(windows) {
        canonical_str
            .strip_prefix("\\\\?\\")
            .unwrap_or(&canonical_str)
            .to_string()
    } else {
        canonical_str.clone()
    };

    // If an allowed root is specified, verify the resolved path is underneath it
    if let Some(root) = allowed_root {
        let root_canonical = std::fs::canonicalize(root).map_err(|e| {
            FourDaError::Validation(format!("Allowed root path could not be resolved: {e}"))
        })?;
        let root_str = root_canonical.to_string_lossy().to_string();
        let root_normalized = if cfg!(windows) {
            root_str
                .strip_prefix("\\\\?\\")
                .unwrap_or(&root_str)
                .to_string()
        } else {
            root_str.clone()
        };

        if !normalized.starts_with(&root_normalized) {
            tracing::warn!(
                target: "4da::security",
                field,
                resolved = %normalized,
                allowed_root = %root_normalized,
                "Canonical path escapes allowed root"
            );
            return Err(FourDaError::Validation(format!(
                "{field} resolves to a path outside the allowed directory"
            )));
        }
    }

    Ok(normalized)
}

/// Ollama's default local endpoint — explicitly allowed through SSRF checks.
// REMOVE BY 2026-08-01
#[allow(dead_code)]
const OLLAMA_HOST: &str = "127.0.0.1";
// REMOVE BY 2026-08-01
#[allow(dead_code)]
const OLLAMA_PORT: u16 = 11434;

/// Validate a URL is safe for outbound HTTP requests (SSRF prevention).
// REMOVE BY 2026-08-01
#[allow(dead_code)]
///
/// Blocks:
/// - Non-HTTP(S) schemes (file://, ftp://, data:, etc.)
/// - Private/internal IP addresses (RFC 1918, loopback, link-local)
/// - IPv6 loopback and unique-local addresses
/// - URLs containing embedded credentials (`user:pass@host`)
/// - Localhost references (by name or IP)
///
/// Exception: `127.0.0.1:11434` (Ollama) is explicitly allowed.
pub(crate) fn validate_url_safe_for_request(field: &str, url: &str) -> Result<String> {
    // Basic input validation first
    let clean = validate_url_input(field, url)?;

    // Parse the URL
    let parsed = url::Url::parse(&clean).map_err(|e| {
        tracing::warn!(
            target: "4da::security",
            field,
            url = %clean,
            error = %e,
            "Invalid URL format"
        );
        FourDaError::Validation(format!("{field} is not a valid URL"))
    })?;

    // Enforce HTTP(S) scheme only
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            tracing::warn!(
                target: "4da::security",
                field,
                scheme,
                "Non-HTTP scheme blocked"
            );
            return Err(FourDaError::Validation(format!(
                "{field} must use http or https scheme, got '{scheme}'"
            )));
        }
    }

    // Block embedded credentials (user:pass@host)
    if !parsed.username().is_empty() || parsed.password().is_some() {
        tracing::warn!(
            target: "4da::security",
            field,
            "URL contains embedded credentials"
        );
        return Err(FourDaError::Validation(format!(
            "{field} must not contain embedded credentials"
        )));
    }

    // Extract host
    let host = parsed
        .host_str()
        .ok_or_else(|| FourDaError::Validation(format!("{field} has no host")))?;

    let port = parsed.port();

    // Check if this is the Ollama exception before blocking private IPs
    if is_ollama_endpoint(host, port) {
        return Ok(clean);
    }

    // Block localhost references (by name)
    let host_lower = host.to_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") || host_lower == "[::1]" {
        tracing::warn!(
            target: "4da::security",
            field,
            host,
            "Localhost URL blocked (SSRF prevention)"
        );
        return Err(FourDaError::Validation(format!(
            "{field} targets a local address which is not allowed"
        )));
    }

    // Parse and check IP addresses
    // Strip brackets from IPv6 (e.g., [::1] -> ::1)
    let ip_candidate = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = ip_candidate.parse::<std::net::IpAddr>() {
        if is_private_ip(&ip) {
            tracing::warn!(
                target: "4da::security",
                field,
                ip = %ip,
                "Private/internal IP blocked (SSRF prevention)"
            );
            return Err(FourDaError::Validation(format!(
                "{field} targets a private/internal IP address which is not allowed"
            )));
        }
    }

    Ok(clean)
}

/// Check if a host:port pair matches the Ollama local endpoint.
fn is_ollama_endpoint(host: &str, port: Option<u16>) -> bool {
    let host_lower = host.to_lowercase();
    let is_local = host_lower == OLLAMA_HOST
        || host_lower == "localhost"
        || host_lower == "[::1]"
        || host_lower == "::1";
    is_local && port == Some(OLLAMA_PORT)
}

/// Check if an IP address is private/internal (RFC 1918, loopback, link-local, etc.).
fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()             // 127.0.0.0/8
                || v4.is_private()       // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                || v4.is_link_local()    // 169.254.0.0/16
                || v4.is_unspecified()   // 0.0.0.0
                || v4.is_broadcast()     // 255.255.255.255
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64 // 100.64.0.0/10 (CGNAT)
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()             // ::1
                || v6.is_unspecified()   // ::
                // fc00::/7 — unique local addresses (ULA)
                || (v6.segments()[0] & 0xFE00) == 0xFC00
                // fe80::/10 — link-local
                || (v6.segments()[0] & 0xFFC0) == 0xFE80
        }
    }
}

#[cfg(test)]
#[path = "ipc_guard_tests.rs"]
mod tests;

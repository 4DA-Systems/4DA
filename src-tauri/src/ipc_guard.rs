// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! IPC input validation and rate limiting for Tauri commands.
//!
//! Provides reusable validation functions for high-risk IPC endpoints
//! (file paths, path components, URLs, search queries, large text inputs).
//!
//! Two path guards live here and they are NOT interchangeable — picking the
//! wrong one is how traversal bugs get written:
//!
//! | Guard | Input shape | Accepts separators / absolute? |
//! |---|---|---|
//! | [`validate_path_input`] | a WHOLE path the user chose (a project directory) | yes, by design |
//! | [`validate_path_component`] | ONE segment spliced into a path we build | no, never |

use crate::error::{FourDaError, Result};

/// Maximum length for general string inputs (search queries, names, labels)
pub const MAX_INPUT_LENGTH: usize = 10_000;

/// Maximum length for content/body inputs (feedback text, descriptions)
pub const MAX_CONTENT_LENGTH: usize = 50_000;

/// Maximum length for URL inputs
pub const MAX_URL_LENGTH: usize = 2_048;

/// Maximum length for file path inputs
pub const MAX_PATH_LENGTH: usize = 1_024;

/// Maximum length for a single filesystem path component (one directory or
/// file-name segment). Deliberately far tighter than [`MAX_PATH_LENGTH`]:
/// every legitimate component in this codebase (locale codes, translation
/// namespaces, sha256 identity hashes) is comfortably under this.
pub const MAX_PATH_COMPONENT_LENGTH: usize = 64;

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

/// The character class considered safe for a single filesystem path
/// component: ASCII alphanumerics plus `_` and `-`.
///
/// Single source of truth for "safe component character". Two policies are
/// built on it and they are deliberately different:
///
///   - [`validate_path_component`] **rejects** anything outside the class. Use
///     it for values that cross a trust boundary (IPC parameters).
///   - `calibration_store::sanitize_path_component` **replaces** anything
///     outside the class with `_`. Use it only for internally-derived values
///     (hashes, hardcoded task names) where a lossy mapping surprises no one.
pub(crate) fn is_safe_component_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Validate that a string is safe to use as a **single** filesystem path
/// component — a value spliced into a path we build, via `Path::join(value)`
/// or `format!("{value}.json")`.
///
/// Rejects empty, over-long, and every character outside
/// [`is_safe_component_char`]. That single character-class check subsumes NUL
/// bytes, control characters, both path separators, `..` traversal, `:` (drive
/// letters and NTFS alternate data streams), leading `~`, and every non-ASCII
/// homoglyph — without needing a blocklist that has to anticipate each one.
///
/// Rejecting rather than sanitizing matters at a trust boundary: `Path::join`
/// with an **absolute** component *replaces* the accumulated path instead of
/// appending to it, so traversal is not even required to escape — and a caller
/// cannot distinguish "wrote where you asked" from "silently wrote somewhere
/// else" if the component was quietly mangled instead of refused.
///
/// This is NOT a substitute for [`validate_path_input`], and vice versa; see
/// the module docs for which to use where.
pub(crate) fn validate_path_component(field: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        return Err(FourDaError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    if value.len() > MAX_PATH_COMPONENT_LENGTH {
        tracing::warn!(
            target: "4da::security",
            field,
            len = value.len(),
            max = MAX_PATH_COMPONENT_LENGTH,
            "Path component exceeds maximum length"
        );
        return Err(FourDaError::Validation(format!(
            "{field} exceeds maximum length of {MAX_PATH_COMPONENT_LENGTH} characters"
        )));
    }
    if !value.chars().all(is_safe_component_char) {
        // Deliberately does not echo the offending input back to the caller.
        tracing::warn!(
            target: "4da::security",
            field,
            "Unsafe character in path component — rejected"
        );
        return Err(FourDaError::Validation(format!(
            "{field} may only contain letters, digits, '_' and '-'"
        )));
    }
    Ok(value.to_string())
}

/// Validate a **whole** user-chosen file path: length + no null bytes + no
/// traversal.
///
/// Absolute paths are accepted **by design** — every production caller
/// (`dependency_commands`, `toolkit`) takes a project directory the user picked
/// with a native folder dialog, and those are always absolute. Do not "harden"
/// this by rejecting absolute paths; that breaks the real callers and does not
/// address the actual hazard, which is *path components*. For a value that must
/// be one path segment, use [`validate_path_component`] instead.
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
///
/// Performs all checks from `validate_path_input` plus:
/// - Resolves symlinks via `std::fs::canonicalize()`
/// - Blocks Windows UNC paths (`\\server\share`)
/// - Optionally validates the resolved path is under an allowed root
///
/// Returns the canonicalized path as a string.
///
/// NO PRODUCTION CALLER TODAY — exercised only by `ipc_guard_tests`. Kept as the
/// hardened path-validation primitive any future filesystem-touching IPC command
/// must use; deleting it would invite an unguarded re-implementation. The expired
/// removal marker dated 2026-08-01 was cleared 2026-08-12 rather than rolled
/// forward; wiring or removal is an owner decision.
#[allow(dead_code)] // REMOVE BY 2026-11-12
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
#[allow(dead_code)] // REMOVE BY 2026-11-12
const OLLAMA_HOST: &str = "127.0.0.1";
#[allow(dead_code)] // REMOVE BY 2026-11-12
const OLLAMA_PORT: u16 = 11434;

/// Validate a URL is safe for outbound HTTP requests (SSRF prevention).
///
/// Blocks:
/// - Non-HTTP(S) schemes (file://, ftp://, data:, etc.)
/// - Private/internal IP addresses (RFC 1918, loopback, link-local)
/// - IPv6 loopback and unique-local addresses
/// - URLs containing embedded credentials (`user:pass@host`)
/// - Localhost references (by name or IP)
///
/// Exception: `127.0.0.1:11434` (Ollama) is explicitly allowed.
///
/// WIRED, BUT ONLY BEHIND A NON-DEFAULT FEATURE (corrected 2026-08-15): the one
/// production caller is `webhooks::commands::register_webhook_cmd`, gated on
/// `feature = "enterprise"`. The default build compiles the `webhooks_stub`
/// instead, so under default features this genuinely has no caller — which is
/// why the dead-code allowance below must stay until `enterprise` ships on by
/// default. The previous "NO PRODUCTION CALLER TODAY" note predated that caller
/// and was stale.
///
/// WHY IT IS NOT WIRED MORE WIDELY (recorded 2026-08-13 after an audit flagged
/// it as orphaned hardening):
///
/// This policy is deliberately stricter than 4DA's actual use cases, and
/// wiring it into the outbound-fetch paths as-is WOULD BREAK legitimate,
/// documented behaviour:
///   - Self-hosted / private-network RSS feeds. 4DA lets a developer add any
///     feed URL; a homelab Gitea, an internal Confluence, or a LAN service is
///     a normal thing to follow. `is_private_ip` rejects all of them.
///   - Local LLM endpoints other than Ollama's default. Settings expose a
///     custom OpenAI-compatible `baseUrl`; LM Studio, llama.cpp servers and
///     proxies bind to other ports, and the exemption here is hardcoded to
///     127.0.0.1:11434.
///
/// So this is NOT dead code to delete, and NOT a gate to switch on blindly.
/// If SSRF hardening is wanted on the fetch path, it needs a policy that
/// distinguishes user-authored URLs (a feed the user typed — trusted) from
/// content-derived URLs (a link discovered inside fetched content — untrusted,
/// and the real SSRF vector). Apply it to the latter only.
#[allow(dead_code)] // REMOVE BY 2026-11-12
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

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
#[allow(dead_code)]
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

/// Validate a URL input: length + no null bytes + valid URL format.
#[allow(dead_code)]
pub(crate) fn validate_url_input(field: &str, url: &str) -> Result<String> {
    let clean = validate_length(field, url, MAX_URL_LENGTH)?;
    validate_no_null_bytes(field, &clean)?;
    // URL format validation is done by utils::validate_safe_url when needed
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
const OLLAMA_HOST: &str = "127.0.0.1";
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
mod tests {
    use super::*;

    // === Existing tests ===

    #[test]
    fn test_validate_length_ok() {
        assert!(validate_length("test", "hello", 100).is_ok());
    }

    #[test]
    fn test_validate_length_too_long() {
        let long = "a".repeat(101);
        assert!(validate_length("test", &long, 100).is_err());
    }

    #[test]
    fn test_validate_length_trims() {
        let result = validate_length("test", "  hello  ", 100).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_validate_no_null_bytes() {
        assert!(validate_no_null_bytes("test", "hello").is_ok());
        assert!(validate_no_null_bytes("test", "hel\0lo").is_err());
    }

    #[test]
    fn test_validate_path_no_traversal() {
        assert!(validate_path_input("path", "/safe/path/file.txt").is_ok());
        assert!(validate_path_input("path", "/unsafe/../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_url_length() {
        assert!(validate_url_input("url", "https://example.com").is_ok());
        let long_url = format!("https://example.com/{}", "a".repeat(2100));
        assert!(validate_url_input("url", &long_url).is_err());
    }

    // === Canonical path validation tests ===

    #[test]
    fn test_canonical_path_resolves_real_path() {
        // Use a path we know exists (the project directory)
        let result = validate_path_canonical("path", ".", None);
        assert!(result.is_ok(), "Should resolve '.' to canonical path");
        let resolved = result.unwrap();
        assert!(
            !resolved.contains(".."),
            "Canonical path should not contain '..'"
        );
    }

    #[test]
    fn test_canonical_path_blocks_traversal() {
        // The basic validate_path_input check catches ".." before canonicalize
        let result = validate_path_canonical("path", "/tmp/../etc/passwd", None);
        assert!(result.is_err(), "Should block path traversal");
    }

    #[test]
    fn test_canonical_path_blocks_unc_paths() {
        let result = validate_path_canonical("path", "\\\\server\\share\\file.txt", None);
        assert!(result.is_err(), "Should block UNC paths with backslashes");

        let result = validate_path_canonical("path", "//server/share/file.txt", None);
        assert!(
            result.is_err(),
            "Should block UNC paths with forward slashes"
        );
    }

    #[test]
    fn test_canonical_path_blocks_nonexistent() {
        let result =
            validate_path_canonical("path", "/nonexistent_4da_test_path_xyz123/file.txt", None);
        assert!(result.is_err(), "Should fail for nonexistent paths");
    }

    #[test]
    fn test_canonical_path_enforces_allowed_root() {
        // Create a temp directory structure for testing
        let temp = std::env::temp_dir();
        let test_dir = temp.join("4da_ipc_guard_test_root");
        let _ = std::fs::create_dir_all(&test_dir);
        let test_file = test_dir.join("allowed.txt");
        let _ = std::fs::write(&test_file, "test");

        // Path inside allowed root should succeed
        let result = validate_path_canonical("path", &test_file.to_string_lossy(), Some(&test_dir));
        assert!(
            result.is_ok(),
            "Path inside allowed root should be accepted"
        );

        // Clean up
        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir(&test_dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_canonical_path_resolves_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = std::env::temp_dir();
        let real_dir = temp.join("4da_ipc_guard_real");
        let link_path = temp.join("4da_ipc_guard_symlink");
        let _ = std::fs::create_dir_all(&real_dir);
        let real_file = real_dir.join("secret.txt");
        let _ = std::fs::write(&real_file, "secret");

        // Create symlink pointing to real_dir
        let _ = std::fs::remove_file(&link_path);
        if symlink(&real_dir, &link_path).is_ok() {
            let link_file = link_path.join("secret.txt");

            // Allowed root is a different directory — symlink escapes it
            let safe_root = temp.join("4da_ipc_guard_safe_root");
            let _ = std::fs::create_dir_all(&safe_root);

            let result =
                validate_path_canonical("path", &link_file.to_string_lossy(), Some(&safe_root));
            assert!(
                result.is_err(),
                "Symlink resolving outside allowed root should be blocked"
            );

            let _ = std::fs::remove_dir(&safe_root);
            let _ = std::fs::remove_file(&link_path);
        }

        let _ = std::fs::remove_file(&real_file);
        let _ = std::fs::remove_dir(&real_dir);
    }

    #[cfg(windows)]
    #[test]
    fn test_canonical_path_resolves_symlinks_windows() {
        // On Windows, symlink creation often requires elevated privileges,
        // so we test with junction points or just verify canonicalize works
        // with a normal directory structure
        let temp = std::env::temp_dir();
        let test_dir = temp.join("4da_ipc_guard_win_test");
        let _ = std::fs::create_dir_all(&test_dir);
        let test_file = test_dir.join("test.txt");
        let _ = std::fs::write(&test_file, "test");

        let result = validate_path_canonical("path", &test_file.to_string_lossy(), Some(&test_dir));
        assert!(
            result.is_ok(),
            "Normal path within root should succeed on Windows"
        );

        // Verify the result is a clean path (no \\?\ prefix)
        let resolved = result.unwrap();
        assert!(
            !resolved.starts_with("\\\\?\\"),
            "Should strip extended-length prefix on Windows"
        );

        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir(&test_dir);
    }

    // === SSRF prevention tests ===

    #[test]
    fn test_ssrf_blocks_private_ipv4() {
        // 10.x.x.x
        assert!(
            validate_url_safe_for_request("url", "https://10.0.0.1/api").is_err(),
            "Should block 10.0.0.0/8"
        );
        // 172.16-31.x.x
        assert!(
            validate_url_safe_for_request("url", "https://172.16.0.1/api").is_err(),
            "Should block 172.16.0.0/12"
        );
        assert!(
            validate_url_safe_for_request("url", "https://172.31.255.255/api").is_err(),
            "Should block upper range of 172.16.0.0/12"
        );
        // 192.168.x.x
        assert!(
            validate_url_safe_for_request("url", "https://192.168.1.1/api").is_err(),
            "Should block 192.168.0.0/16"
        );
    }

    #[test]
    fn test_ssrf_blocks_loopback() {
        assert!(
            validate_url_safe_for_request("url", "https://127.0.0.1/api").is_err(),
            "Should block 127.0.0.1"
        );
        assert!(
            validate_url_safe_for_request("url", "https://127.0.0.2/api").is_err(),
            "Should block 127.0.0.2"
        );
        assert!(
            validate_url_safe_for_request("url", "https://localhost/api").is_err(),
            "Should block localhost"
        );
    }

    #[test]
    fn test_ssrf_blocks_ipv6_private() {
        assert!(
            validate_url_safe_for_request("url", "https://[::1]/api").is_err(),
            "Should block IPv6 loopback"
        );
        assert!(
            validate_url_safe_for_request("url", "https://[fc00::1]/api").is_err(),
            "Should block fc00::/7 unique local"
        );
        assert!(
            validate_url_safe_for_request("url", "https://[fd12::1]/api").is_err(),
            "Should block fd00::/8 (within fc00::/7)"
        );
        assert!(
            validate_url_safe_for_request("url", "https://[fe80::1]/api").is_err(),
            "Should block fe80::/10 link-local"
        );
    }

    #[test]
    fn test_ssrf_blocks_non_http_schemes() {
        assert!(
            validate_url_safe_for_request("url", "file:///etc/passwd").is_err(),
            "Should block file:// scheme"
        );
        assert!(
            validate_url_safe_for_request("url", "ftp://ftp.example.com/file").is_err(),
            "Should block ftp:// scheme"
        );
        assert!(
            validate_url_safe_for_request("url", "gopher://evil.com/").is_err(),
            "Should block gopher:// scheme"
        );
        assert!(
            validate_url_safe_for_request("url", "data:text/html,<h1>hi</h1>").is_err(),
            "Should block data: scheme"
        );
    }

    #[test]
    fn test_ssrf_blocks_credentials_in_url() {
        assert!(
            validate_url_safe_for_request("url", "https://user:pass@example.com/api").is_err(),
            "Should block URLs with user:pass credentials"
        );
        assert!(
            validate_url_safe_for_request("url", "https://admin@example.com/api").is_err(),
            "Should block URLs with username only"
        );
    }

    #[test]
    fn test_ssrf_allows_public_urls() {
        assert!(
            validate_url_safe_for_request("url", "https://api.github.com/repos").is_ok(),
            "Should allow public HTTPS URLs"
        );
        assert!(
            validate_url_safe_for_request("url", "http://example.com/feed.xml").is_ok(),
            "Should allow public HTTP URLs"
        );
        assert!(
            validate_url_safe_for_request("url", "https://8.8.8.8/dns-query").is_ok(),
            "Should allow public IP addresses"
        );
    }

    #[test]
    fn test_ssrf_allows_ollama_exception() {
        assert!(
            validate_url_safe_for_request("url", "http://127.0.0.1:11434/api/embeddings").is_ok(),
            "Should allow Ollama at 127.0.0.1:11434"
        );
        assert!(
            validate_url_safe_for_request("url", "http://localhost:11434/api/generate").is_ok(),
            "Should allow Ollama at localhost:11434"
        );
    }

    #[test]
    fn test_ssrf_blocks_ollama_port_on_wrong_host() {
        // Port 11434 on a private IP that isn't localhost should still be blocked
        assert!(
            validate_url_safe_for_request("url", "http://10.0.0.1:11434/api").is_err(),
            "Should block Ollama port on non-local private IP"
        );
    }

    #[test]
    fn test_ssrf_blocks_localhost_wrong_port() {
        // localhost on a port that isn't Ollama should be blocked
        assert!(
            validate_url_safe_for_request("url", "http://localhost:8080/api").is_err(),
            "Should block localhost on non-Ollama port"
        );
        assert!(
            validate_url_safe_for_request("url", "http://127.0.0.1:9090/api").is_err(),
            "Should block loopback on non-Ollama port"
        );
    }

    #[test]
    fn test_ssrf_blocks_cgnat_range() {
        assert!(
            validate_url_safe_for_request("url", "https://100.64.0.1/api").is_err(),
            "Should block 100.64.0.0/10 CGNAT range"
        );
        assert!(
            validate_url_safe_for_request("url", "https://100.127.255.254/api").is_err(),
            "Should block upper CGNAT range"
        );
    }

    // === Private IP helper tests ===

    #[test]
    fn test_is_private_ip() {
        use std::net::IpAddr;

        // Private IPv4
        assert!(is_private_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"192.168.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"169.254.1.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"0.0.0.0".parse::<IpAddr>().unwrap()));

        // Public IPv4
        assert!(!is_private_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(&"93.184.216.34".parse::<IpAddr>().unwrap()));

        // Private IPv6
        assert!(is_private_ip(&"::1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"fc00::1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"fd12:3456::1".parse::<IpAddr>().unwrap()));
        assert!(is_private_ip(&"fe80::1".parse::<IpAddr>().unwrap()));

        // Public IPv6
        assert!(!is_private_ip(&"2001:db8::1".parse::<IpAddr>().unwrap()));
        assert!(!is_private_ip(
            &"2607:f8b0:4004:800::200e".parse::<IpAddr>().unwrap()
        ));
    }

    // === UNC path tests ===

    #[test]
    fn test_unc_paths_blocked_in_canonical() {
        // Double backslash UNC
        let result = validate_path_canonical("path", "\\\\evil-server\\share\\secrets", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("UNC"), "Error should mention UNC: {err}");

        // Double forward slash UNC
        let result = validate_path_canonical("path", "//evil-server/share/secrets", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_unc_path_not_caught_by_basic_validate() {
        // validate_path_input does NOT catch UNC — that's why validate_path_canonical exists
        let result = validate_path_input("path", "\\\\server\\share");
        assert!(result.is_ok(), "Basic validation doesn't catch UNC");
    }
}

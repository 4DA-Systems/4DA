// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! URL validation for safe external opening.
//!
//! Prevents protocol injection attacks (file://, javascript:, ms-msdt:, etc.)
//! by enforcing an allow-list of safe URL schemes before opening in the system browser.

use crate::error::{FourDaError, Result};

/// Allowed URL schemes for opening in the system browser.
const SAFE_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Validate that a URL is safe to open in the system browser.
///
/// Only allows http://, https://, and mailto: schemes.
/// Rejects file://, javascript:, data:, vbscript:, and OS-specific protocol handlers.
pub(crate) fn validate_safe_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(FourDaError::Validation("URL is empty".into()));
    }

    // Parse the URL to extract the scheme
    match url::Url::parse(trimmed) {
        Ok(parsed) => {
            let scheme = parsed.scheme().to_lowercase();
            if SAFE_SCHEMES.contains(&scheme.as_str()) {
                Ok(())
            } else {
                tracing::warn!(
                    target: "4da::security",
                    scheme = %scheme,
                    "Blocked URL with disallowed scheme"
                );
                Err(FourDaError::Validation(format!(
                    "URL scheme '{scheme}' is not allowed — only http, https, and mailto links can be opened"
                )))
            }
        }
        Err(_) => Err(FourDaError::Validation("Invalid URL format".into())),
    }
}

/// Validate that a deep-link URL matches the expected 4DA protocol.
///
/// Only allows `4da://` scheme with known host paths.
///
/// Note: `4da://` is not a valid RFC 3986 scheme (starts with a digit), so
/// `url::Url::parse` rejects it. We use string-based validation instead since
/// this is a custom OS-registered protocol handler.
pub(crate) fn validate_deep_link_url(url: &str) -> bool {
    let trimmed = url.trim();

    // Must start with the 4da:// scheme (case-insensitive)
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("4da://") {
        if !trimmed.is_empty() {
            tracing::warn!(
                target: "4da::security",
                url = %trimmed,
                "Rejected deep-link with unexpected scheme"
            );
        }
        return false;
    }

    // Extract the host portion (everything between 4da:// and the first / or ?)
    let after_scheme = &trimmed[6..]; // len("4da://") == 6
    let host = after_scheme
        .split(|c: char| c == '/' || c == '?')
        .next()
        .unwrap_or("");

    matches!(host, "activate" | "open" | "settings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_https_url() {
        assert!(validate_safe_url("https://example.com").is_ok());
    }

    #[test]
    fn test_valid_http_url() {
        assert!(validate_safe_url("http://example.com/path?q=1").is_ok());
    }

    #[test]
    fn test_valid_mailto_url() {
        assert!(validate_safe_url("mailto:user@example.com").is_ok());
    }

    #[test]
    fn test_reject_file_scheme() {
        let err = validate_safe_url("file:///C:/Windows/System32/cmd.exe");
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn test_reject_javascript_scheme() {
        let err = validate_safe_url("javascript:alert(1)");
        assert!(err.is_err());
    }

    #[test]
    fn test_reject_data_scheme() {
        let err = validate_safe_url("data:text/html,<script>alert(1)</script>");
        assert!(err.is_err());
    }

    #[test]
    fn test_reject_empty_url() {
        assert!(validate_safe_url("").is_err());
        assert!(validate_safe_url("   ").is_err());
    }

    #[test]
    fn test_reject_malformed_url() {
        assert!(validate_safe_url("not a url at all").is_err());
    }

    #[test]
    fn test_reject_windows_protocol_handlers() {
        assert!(validate_safe_url("ms-msdt:/id").is_err());
        assert!(validate_safe_url("search-ms:query=malware").is_err());
        assert!(validate_safe_url("ms-officecmd:something").is_err());
        assert!(validate_safe_url("vbscript:msgbox").is_err());
    }

    #[test]
    fn test_valid_deep_link() {
        assert!(validate_deep_link_url("4da://activate?key=abc123"));
    }

    #[test]
    fn test_reject_non_4da_deep_link() {
        assert!(!validate_deep_link_url("http://evil.com/activate"));
        assert!(!validate_deep_link_url("4da://unknown-host"));
        assert!(!validate_deep_link_url(""));
    }
}

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
                // Log to security audit trail
                if let Ok(db) = crate::get_database() {
                    db.log_security_event("url_blocked", &format!("scheme: {scheme}"), "warning");
                }
                Err(FourDaError::Validation(format!(
                    "URL scheme '{scheme}' is not allowed — only http, https, and mailto links can be opened"
                )))
            }
        }
        Err(_) => Err(FourDaError::Validation("Invalid URL format".into())),
    }
}

/// Maximum allowed length for deep-link URLs (prevents buffer-based attacks).
const DEEP_LINK_MAX_LENGTH: usize = 2048;

/// Maximum allowed length for a single query parameter value.
const DEEP_LINK_MAX_PARAM_VALUE_LENGTH: usize = 512;

/// Characters forbidden in query parameter values (injection prevention).
const FORBIDDEN_VALUE_CHARS: &[char] = &['<', '>', '"', '\'', ';', '|', '&', '`'];

/// Validate that a deep-link URL matches the expected 4DA protocol.
///
/// Only allows `4da://` scheme with known host paths. Applies additional
/// hardening against injection attacks:
/// - Length limit (2048 chars)
/// - Null byte rejection
/// - Query parameter key/value sanitization
/// - Path traversal detection
///
/// Note: `4da://` is not a valid RFC 3986 scheme (starts with a digit), so
/// `url::Url::parse` rejects it. We use string-based validation instead since
/// this is a custom OS-registered protocol handler.
pub(crate) fn validate_deep_link_url(url: &str) -> bool {
    let trimmed = url.trim();

    // Length limit — reject oversized URLs before any parsing
    if trimmed.len() > DEEP_LINK_MAX_LENGTH {
        tracing::warn!(
            target: "4da::security",
            len = trimmed.len(),
            max = DEEP_LINK_MAX_LENGTH,
            "Rejected deep-link: URL exceeds maximum length"
        );
        if let Ok(db) = crate::get_database() {
            db.log_security_event("deeplink_blocked", "URL exceeds max length", "warning");
        }
        return false;
    }

    // Null byte check — prevents null-byte injection attacks
    if trimmed.contains('\0') {
        tracing::warn!(
            target: "4da::security",
            "Rejected deep-link: URL contains null byte"
        );
        if let Ok(db) = crate::get_database() {
            db.log_security_event("deeplink_blocked", "null byte in URL", "warning");
        }
        return false;
    }

    // Must start with the 4da:// scheme (case-insensitive)
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("4da://") {
        if !trimmed.is_empty() {
            tracing::warn!(
                target: "4da::security",
                url = %trimmed,
                "Rejected deep-link with unexpected scheme"
            );
            if let Ok(db) = crate::get_database() {
                db.log_security_event(
                    "deeplink_blocked",
                    &format!("bad scheme: {trimmed}"),
                    "warning",
                );
            }
        }
        return false;
    }

    // Extract the host portion (everything between 4da:// and the first / or ?)
    let after_scheme = &trimmed[6..]; // len("4da://") == 6
    let host = after_scheme
        .split(|c: char| c == '/' || c == '?')
        .next()
        .unwrap_or("");

    if !matches!(host, "activate" | "open" | "settings") {
        tracing::warn!(
            target: "4da::security",
            host = %host,
            "Rejected deep-link: unknown host"
        );
        if let Ok(db) = crate::get_database() {
            db.log_security_event(
                "deeplink_blocked",
                &format!("unknown host: {host}"),
                "warning",
            );
        }
        return false;
    }

    // Path traversal check — reject any segment containing ".."
    let path_and_query = &after_scheme[host.len()..];
    let path = path_and_query.split('?').next().unwrap_or("");
    if path
        .split('/')
        .any(|segment| segment == ".." || segment == ".")
    {
        tracing::warn!(
            target: "4da::security",
            path = %path,
            "Rejected deep-link: path traversal detected"
        );
        if let Ok(db) = crate::get_database() {
            db.log_security_event("deeplink_blocked", "path traversal attempt", "warning");
        }
        return false;
    }

    // Query parameter validation
    if let Some(query_str) = path_and_query.split('?').nth(1) {
        for pair in query_str.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (key, value) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };

            // Keys must be alphanumeric + underscore only
            if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                tracing::warn!(
                    target: "4da::security",
                    key = %key,
                    "Rejected deep-link: query key contains invalid characters"
                );
                if let Ok(db) = crate::get_database() {
                    db.log_security_event(
                        "deeplink_blocked",
                        &format!("invalid query key: {key}"),
                        "warning",
                    );
                }
                return false;
            }

            // Value length limit
            if value.len() > DEEP_LINK_MAX_PARAM_VALUE_LENGTH {
                tracing::warn!(
                    target: "4da::security",
                    key = %key,
                    len = value.len(),
                    "Rejected deep-link: query value exceeds max length"
                );
                if let Ok(db) = crate::get_database() {
                    db.log_security_event(
                        "deeplink_blocked",
                        &format!("query value too long for key: {key}"),
                        "warning",
                    );
                }
                return false;
            }

            // Forbidden characters in values (injection prevention)
            if value.contains(FORBIDDEN_VALUE_CHARS) {
                tracing::warn!(
                    target: "4da::security",
                    key = %key,
                    "Rejected deep-link: query value contains forbidden characters"
                );
                if let Ok(db) = crate::get_database() {
                    db.log_security_event(
                        "deeplink_blocked",
                        &format!("forbidden chars in value for key: {key}"),
                        "warning",
                    );
                }
                return false;
            }
        }
    }

    true
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
        assert!(validate_deep_link_url("4da://open?source=hackernews"));
        assert!(validate_deep_link_url("4da://settings"));
        assert!(validate_deep_link_url("4da://activate"));
    }

    #[test]
    fn test_valid_deep_link_with_path() {
        assert!(validate_deep_link_url("4da://activate/license?key=abc123"));
    }

    #[test]
    fn test_reject_non_4da_deep_link() {
        assert!(!validate_deep_link_url("http://evil.com/activate"));
        assert!(!validate_deep_link_url("4da://unknown-host"));
        assert!(!validate_deep_link_url(""));
    }

    #[test]
    fn test_reject_deep_link_too_long() {
        let long_url = format!("4da://activate?key={}", "a".repeat(2048));
        assert!(!validate_deep_link_url(&long_url));
    }

    #[test]
    fn test_reject_deep_link_null_byte() {
        assert!(!validate_deep_link_url("4da://activate?key=abc\0def"));
        assert!(!validate_deep_link_url("4da://activate\0/../evil"));
    }

    #[test]
    fn test_reject_deep_link_script_injection() {
        assert!(!validate_deep_link_url(
            "4da://activate?key=<script>alert(1)</script>"
        ));
        assert!(!validate_deep_link_url(
            "4da://open?url=<img onerror=alert(1)>"
        ));
    }

    #[test]
    fn test_reject_deep_link_shell_metacharacters() {
        assert!(!validate_deep_link_url("4da://activate?cmd=rm;ls"));
        assert!(!validate_deep_link_url("4da://activate?cmd=cat|grep"));
        assert!(!validate_deep_link_url(
            "4da://activate?cmd=foo&bar=baz`whoami`"
        ));
    }

    #[test]
    fn test_reject_deep_link_special_chars_in_key() {
        assert!(!validate_deep_link_url("4da://activate?ke<y=value"));
        assert!(!validate_deep_link_url("4da://activate?k.ey=value"));
        assert!(!validate_deep_link_url("4da://activate?ke-y=value"));
        assert!(!validate_deep_link_url("4da://activate?ke%20y=value"));
    }

    #[test]
    fn test_reject_deep_link_path_traversal() {
        assert!(!validate_deep_link_url(
            "4da://activate/../../../etc/passwd"
        ));
        assert!(!validate_deep_link_url("4da://open/./hidden"));
    }

    #[test]
    fn test_reject_deep_link_long_param_value() {
        let long_value = "a".repeat(513);
        assert!(!validate_deep_link_url(&format!(
            "4da://activate?key={long_value}"
        )));
    }
}

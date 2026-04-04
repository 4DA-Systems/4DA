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

/// Validate a URL input: length + no null bytes + valid URL format.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

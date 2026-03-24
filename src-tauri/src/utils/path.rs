// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

// ============================================================================
// Path Sanitization (for error messages sent to frontend)
// ============================================================================

/// Sanitize a filesystem path for inclusion in error messages sent to the frontend.
/// Strips the user's home directory prefix and shows only the last 2 path components.
/// Internal logs should still use full paths; this is only for IPC-facing errors.
pub(crate) fn sanitize_path(path: &str) -> String {
    // Normalize separators for comparison
    let normalized = path.replace('\\', "/");

    // Try to strip home directory prefix
    let stripped = if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().replace('\\', "/");
        if let Some(rest) = normalized.strip_prefix(&home_str) {
            format!("...{rest}")
        } else {
            normalized.clone()
        }
    } else {
        normalized.clone()
    };

    // If still long, keep only last 2 path components
    let parts: Vec<&str> = stripped.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() > 2 {
        format!(".../{}/{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        stripped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_path_keeps_short_paths() {
        let result = sanitize_path("src/main.rs");
        assert!(result.contains("main.rs"));
    }

    #[test]
    fn test_sanitize_path_truncates_deep_paths() {
        let result = sanitize_path("/very/deep/nested/directory/structure/src/main.rs");
        assert_eq!(result, ".../src/main.rs");
    }

    #[test]
    fn test_sanitize_path_handles_windows_separators() {
        let result = sanitize_path("C:\\Users\\john\\projects\\myapp\\src\\main.rs");
        // Should normalize and truncate
        assert!(result.contains("main.rs"));
        assert!(!result.contains("john"), "Should not reveal username");
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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

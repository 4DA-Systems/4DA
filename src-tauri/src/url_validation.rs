// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! URL validation utilities — SSRF prevention for user-supplied URLs.
//!
//! Blocks requests to internal/private network addresses to prevent
//! Server-Side Request Forgery (SSRF) attacks.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use crate::error::Result;

/// Known cloud metadata hostnames that must be blocked.
const BLOCKED_HOSTNAMES: &[&str] = &[
    "metadata.google.internal",
    "metadata.google",
    "169.254.169.254",
    "100.100.100.200", // Alibaba Cloud metadata
    "fd00:ec2::254",   // AWS IPv6 metadata
];

/// Check if an IP address is in a private/internal range.
fn is_internal_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_internal_ipv4(v4),
        IpAddr::V6(v6) => is_internal_ipv6(v6),
    }
}

/// Check if an IPv4 address is private, loopback, link-local, or otherwise internal.
fn is_internal_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();

    // 0.0.0.0
    if ip.is_unspecified() {
        return true;
    }

    // 127.0.0.0/8 — loopback
    if ip.is_loopback() {
        return true;
    }

    // 10.0.0.0/8 — RFC 1918
    if octets[0] == 10 {
        return true;
    }

    // 172.16.0.0/12 — RFC 1918
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }

    // 192.168.0.0/16 — RFC 1918
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }

    // 169.254.0.0/16 — link-local (includes AWS metadata 169.254.169.254)
    if octets[0] == 169 && octets[1] == 254 {
        return true;
    }

    // 100.64.0.0/10 — Carrier-grade NAT (RFC 6598), includes 100.100.100.200
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return true;
    }

    false
}

/// Check if an IPv6 address is loopback, link-local, or otherwise internal.
fn is_internal_ipv6(ip: Ipv6Addr) -> bool {
    // ::1 — loopback
    if ip.is_loopback() {
        return true;
    }

    // :: — unspecified
    if ip.is_unspecified() {
        return true;
    }

    let segments = ip.segments();

    // fe80::/10 — link-local
    if segments[0] & 0xffc0 == 0xfe80 {
        return true;
    }

    // fc00::/7 — unique local address (RFC 4193)
    if segments[0] & 0xfe00 == 0xfc00 {
        return true;
    }

    // ::ffff:0:0/96 — IPv4-mapped addresses, check the embedded IPv4
    if segments[0..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff {
        let v4 = Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        );
        return is_internal_ipv4(v4);
    }

    false
}

/// Check if a URL points to an internal/private network address.
///
/// This validates by:
/// 1. Checking the hostname string against known internal patterns
/// 2. Attempting DNS resolution and checking resolved IPs against blocked ranges
///
/// Returns `true` if the URL targets an internal address and should be blocked.
pub(crate) fn is_internal_url(url: &str) -> bool {
    // Parse the URL to extract the host
    let host = match extract_host(url) {
        Some(h) => h,
        None => return false, // Can't parse → let the HTTP client handle it
    };

    let host_lower = host.to_lowercase();

    // Check against known blocked hostnames
    if host_lower == "localhost" {
        return true;
    }

    for &blocked in BLOCKED_HOSTNAMES {
        if host_lower == blocked {
            return true;
        }
    }

    // Try to parse the host directly as an IP address
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_internal_ip(ip);
    }

    // Attempt DNS resolution (best-effort, non-blocking is not feasible here
    // but this runs only at validation time, not in hot paths)
    if let Ok(addrs) = format!("{host}:80").to_socket_addrs() {
        for addr in addrs {
            if is_internal_ip(addr.ip()) {
                return true;
            }
        }
    }

    false
}

/// Extract the hostname from a URL string.
/// Handles http:// and https:// URLs, stripping port and path.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;

    // Take everything before the first `/`, `?`, or `#`
    let host_port = after_scheme
        .split('/')
        .next()?
        .split('?')
        .next()?
        .split('#')
        .next()?;

    // Strip port if present (handle IPv6 bracket notation)
    let host = if host_port.starts_with('[') {
        // IPv6: [::1]:8080 → ::1
        host_port
            .strip_prefix('[')
            .and_then(|s| s.split(']').next())
    } else {
        // IPv4 or hostname: example.com:8080 → example.com
        Some(host_port.split(':').next().unwrap_or(host_port))
    }?;

    if host.is_empty() {
        return None;
    }

    Some(host.to_string())
}

/// Validate that a URL does not target internal network addresses.
/// Returns an error with a clear message if the URL is blocked.
pub(crate) fn validate_not_internal(url: &str) -> Result<()> {
    if is_internal_url(url) {
        return Err(
            "URL blocked: cannot target internal/private network addresses (localhost, 10.x.x.x, 172.16-31.x.x, 192.168.x.x, link-local, cloud metadata endpoints)"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ================================================================
    // extract_host tests
    // ================================================================

    #[test]
    fn extract_host_basic() {
        assert_eq!(
            extract_host("https://example.com/feed"),
            Some("example.com".into())
        );
        assert_eq!(
            extract_host("http://example.com:8080/path"),
            Some("example.com".into())
        );
    }

    #[test]
    fn extract_host_ipv6() {
        assert_eq!(extract_host("http://[::1]:8080/feed"), Some("::1".into()));
    }

    #[test]
    fn extract_host_no_scheme() {
        assert_eq!(extract_host("example.com/feed"), None);
    }

    // ================================================================
    // is_internal_url tests
    // ================================================================

    #[test]
    fn blocks_localhost() {
        assert!(is_internal_url("http://localhost/feed"));
        assert!(is_internal_url("https://localhost:8080/feed"));
        assert!(is_internal_url("http://LOCALHOST/feed"));
    }

    #[test]
    fn blocks_loopback_ipv4() {
        assert!(is_internal_url("http://127.0.0.1/feed"));
        assert!(is_internal_url("http://127.0.0.2/something"));
        assert!(is_internal_url("http://127.255.255.255/rss"));
    }

    #[test]
    fn blocks_loopback_ipv6() {
        assert!(is_internal_url("http://[::1]/feed"));
    }

    #[test]
    fn blocks_rfc1918_10() {
        assert!(is_internal_url("http://10.0.0.1/feed"));
        assert!(is_internal_url("http://10.255.255.255/feed"));
    }

    #[test]
    fn blocks_rfc1918_172() {
        assert!(is_internal_url("http://172.16.0.1/feed"));
        assert!(is_internal_url("http://172.31.255.255/feed"));
        // 172.15 and 172.32 should NOT be blocked
        assert!(!is_internal_url("http://172.15.0.1/feed"));
        assert!(!is_internal_url("http://172.32.0.1/feed"));
    }

    #[test]
    fn blocks_rfc1918_192_168() {
        assert!(is_internal_url("http://192.168.1.1/feed"));
        assert!(is_internal_url("http://192.168.0.0/feed"));
    }

    #[test]
    fn blocks_link_local() {
        assert!(is_internal_url("http://169.254.1.1/feed"));
        assert!(is_internal_url("http://169.254.169.254/latest/meta-data/")); // AWS metadata
    }

    #[test]
    fn blocks_cloud_metadata() {
        assert!(is_internal_url("http://169.254.169.254/latest/meta-data/"));
        assert!(is_internal_url(
            "http://metadata.google.internal/computeMetadata/v1/"
        ));
        assert!(is_internal_url("http://100.100.100.200/latest/meta-data/"));
    }

    #[test]
    fn blocks_zero_address() {
        assert!(is_internal_url("http://0.0.0.0/feed"));
    }

    #[test]
    fn allows_public_urls() {
        assert!(!is_internal_url("https://blog.rust-lang.org/feed.xml"));
        assert!(!is_internal_url("https://hnrss.org/frontpage"));
        assert!(!is_internal_url("http://feeds.feedburner.com/example"));
        assert!(!is_internal_url("https://www.reddit.com/.rss"));
    }

    #[test]
    fn validate_not_internal_error_message() {
        let err = validate_not_internal("http://127.0.0.1/feed").unwrap_err();
        assert!(err.to_string().contains("internal/private"));
    }

    #[test]
    fn validate_not_internal_allows_public() {
        assert!(validate_not_internal("https://example.com/feed.xml").is_ok());
    }

    // ================================================================
    // IP range boundary tests
    // ================================================================

    #[test]
    fn ipv4_boundary_cases() {
        // Just inside 172.16.0.0/12
        assert!(is_internal_ipv4(Ipv4Addr::new(172, 16, 0, 0)));
        assert!(is_internal_ipv4(Ipv4Addr::new(172, 31, 255, 255)));
        // Just outside
        assert!(!is_internal_ipv4(Ipv4Addr::new(172, 15, 255, 255)));
        assert!(!is_internal_ipv4(Ipv4Addr::new(172, 32, 0, 0)));
    }

    #[test]
    fn ipv6_link_local() {
        // fe80::/10
        assert!(is_internal_ipv6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)));
        // febf:: is still in fe80::/10 range (0xfebf & 0xffc0 == 0xfe80)
        assert!(is_internal_ipv6(Ipv6Addr::new(0xfebf, 0, 0, 0, 0, 0, 0, 1)));
    }

    #[test]
    fn ipv6_unique_local() {
        // fc00::/7
        assert!(is_internal_ipv6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)));
        assert!(is_internal_ipv6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)));
    }

    #[test]
    fn ipv4_mapped_ipv6() {
        // ::ffff:127.0.0.1 should be blocked
        let mapped = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x0001);
        assert!(is_internal_ipv6(mapped));

        // ::ffff:8.8.8.8 should NOT be blocked
        let public_mapped = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0808, 0x0808);
        assert!(!is_internal_ipv6(public_mapped));
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! URL validation utilities — SSRF prevention for user-supplied URLs.
//!
//! Blocks requests to internal/private network addresses to prevent
//! Server-Side Request Forgery (SSRF) attacks.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use url::{Host, Url};

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

/// Error message emitted when a URL resolves to an internal address.
pub(crate) const INTERNAL_URL_BLOCKED: &str = "URL blocked: cannot target internal/private network addresses (localhost, 10.x.x.x, 172.16-31.x.x, 192.168.x.x, link-local, cloud metadata endpoints)";

/// Error message emitted when a URL carries embedded credentials.
pub(crate) const CREDENTIALS_IN_URL_BLOCKED: &str =
    "URL blocked: embedded credentials (`user:pass@host`) are not permitted — they mask the real host from readers and leak on redirect. Use an authenticated proxy or a token header instead.";

/// Check if a URL points to an internal/private network address.
///
/// Parsing is delegated to the `url` crate (WHATWG URL Standard, the same
/// parser `reqwest` uses) so that userinfo, IDN, IPv6 brackets, and the
/// obfuscated IPv4 forms (`http://2130706433/`, `http://0x7f.1/`) are handled
/// identically here and at request time. A hand-rolled parser gave
/// `http://evil.com@127.0.0.1/` a host of `evil.com@127.0.0.1`, which failed
/// the IP parse and sailed through the guard.
///
/// Returns `true` if the URL targets an internal address and should be blocked.
///
/// Production call sites go through `validate_not_internal` (which also
/// rejects credentials) or, per redirect hop, through
/// `is_internal_parsed_url`; this string-taking form exists for the tests
/// that assert the parser's behaviour directly.
#[cfg(test)]
fn is_internal_url(url: &str) -> bool {
    match Url::parse(url) {
        Ok(parsed) => is_internal_parsed_url(&parsed),
        // Can't parse → nothing here can be a request either; let the HTTP
        // client reject it.
        Err(_) => false,
    }
}

/// Check whether an already-parsed URL points at an internal address.
///
/// Used directly by the redirect policy in `http_client`, which receives a
/// parsed `Url` per hop and must not pay for a re-parse.
pub(crate) fn is_internal_parsed_url(url: &Url) -> bool {
    match url.host() {
        Some(Host::Ipv4(v4)) => is_internal_ipv4(v4),
        Some(Host::Ipv6(v6)) => is_internal_ipv6(v6),
        Some(Host::Domain(domain)) => is_internal_domain(domain),
        // Schemes without an authority (`mailto:`, `data:`) have no host to
        // reach; they are rejected as non-HTTP upstream.
        None => false,
    }
}

/// Check whether a hostname is internal, by name pattern then by DNS.
fn is_internal_domain(domain: &str) -> bool {
    // Strip the root label (`localhost.` == `localhost`) and normalise case.
    let host_lower = domain.trim_end_matches('.').to_lowercase();

    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return true;
    }

    if BLOCKED_HOSTNAMES.contains(&host_lower.as_str()) {
        return true;
    }

    // A bare literal that the URL parser left as a domain (non-special
    // schemes keep the host verbatim) may still be an IP.
    if let Ok(ip) = host_lower.parse::<IpAddr>() {
        return is_internal_ip(ip);
    }

    // Attempt DNS resolution (best-effort, non-blocking is not feasible here
    // but this runs only at validation time, not in hot paths)
    if let Ok(addrs) = format!("{host_lower}:80").to_socket_addrs() {
        for addr in addrs {
            if is_internal_ip(addr.ip()) {
                return true;
            }
        }
    }

    false
}

/// Validate that a URL is safe to request: no embedded credentials, and not
/// pointed at an internal network address.
///
/// Returns an error with a clear message if the URL is blocked.
pub(crate) fn validate_not_internal(url: &str) -> Result<()> {
    let parsed = match Url::parse(url) {
        Ok(p) => p,
        // Unparseable input cannot become a request; the HTTP client rejects it.
        Err(_) => return Ok(()),
    };

    // Credentials-in-URL are rejected for the same reason `ipc_guard` rejects
    // them: `http://evil.com@127.0.0.1/` reads as "evil.com" to a human and to
    // any log line, while the request goes to loopback.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(CREDENTIALS_IN_URL_BLOCKED.into());
    }

    if is_internal_parsed_url(&parsed) {
        return Err(INTERNAL_URL_BLOCKED.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ================================================================
    // Host-extraction regressions (the hand-rolled parser's blind spots)
    // ================================================================

    /// The userinfo bypass. The hand-rolled `extract_host` split on `/`, `?`,
    /// `#` and `:` but never on `@`, so this URL yielded a "host" of
    /// `evil.com@127.0.0.1`, which failed `parse::<IpAddr>()`, fell through to
    /// a DNS lookup that also failed, and returned `false` — request allowed.
    #[test]
    fn blocks_userinfo_masking_loopback() {
        assert!(is_internal_url("http://evil.com@127.0.0.1/"));
        assert!(is_internal_url("http://user:pass@127.0.0.1:4446/api/dna"));
        assert!(is_internal_url(
            "http://example.com@169.254.169.254/latest/"
        ));
        assert!(is_internal_url("http://a@[::1]/feed"));
    }

    /// Credentials are refused outright, even against a public host — the
    /// same stance `ipc_guard::validate_url_safe_for_request` already takes.
    #[test]
    fn validate_not_internal_rejects_credentials() {
        let err = validate_not_internal("https://user:pass@example.com/feed.xml").unwrap_err();
        assert!(
            err.to_string().contains("embedded credentials"),
            "unexpected message: {err}"
        );
        assert!(validate_not_internal("https://admin@example.com/feed.xml").is_err());
        // And the masked-loopback case is refused by whichever check fires first.
        assert!(validate_not_internal("http://evil.com@127.0.0.1/").is_err());
    }

    /// WHATWG-legal obfuscated IPv4 literals. The old parser handed these to
    /// `parse::<IpAddr>()`, which rejects them, so they were treated as
    /// hostnames and allowed.
    #[test]
    fn blocks_obfuscated_ipv4_literals() {
        // 2130706433 == 0x7f000001 == 127.0.0.1
        assert!(is_internal_url("http://2130706433/"));
        assert!(is_internal_url("http://0x7f000001/"));
        assert!(is_internal_url("http://127.1/"));
        assert!(is_internal_url("http://0177.0.0.1/"));
    }

    /// RFC 6761 reserves `*.localhost` for loopback.
    #[test]
    fn blocks_localhost_suffix_and_root_label() {
        assert!(is_internal_url("http://api.localhost/feed"));
        assert!(is_internal_url("http://localhost./feed"));
        assert!(is_internal_url("http://LOCALHOST./feed"));
    }

    /// Bracketed IPv6 hosts must not leak their brackets into the IP parse.
    #[test]
    fn ipv6_hosts_parse_without_brackets() {
        assert!(is_internal_url("http://[::1]:8080/feed"));
        assert!(is_internal_url("http://[fd00::1]/feed"));
        assert!(is_internal_url("http://[fe80::1]/feed"));
        assert!(!is_internal_url("http://[2606:4700:4700::1111]/feed"));
    }

    /// Unparseable input is not a request; it must not panic or claim internal.
    #[test]
    fn unparseable_input_is_not_internal() {
        assert!(!is_internal_url("example.com/feed"));
        assert!(!is_internal_url(""));
        assert!(!is_internal_url("::::"));
        assert!(validate_not_internal("example.com/feed").is_ok());
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

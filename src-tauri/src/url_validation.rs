// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! URL validation utilities — SSRF prevention for user-supplied URLs.
//!
//! Blocks requests to internal/private network addresses to prevent
//! Server-Side Request Forgery (SSRF) attacks.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

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

/// Check if an IPv4 address is private, loopback, link-local, or otherwise
/// not a public unicast destination (IANA special-purpose registry).
fn is_internal_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    a == 0                                  // 0.0.0.0/8 — "this network"; 0.x reaches the local host on Linux/macOS
        || a == 10                          // 10.0.0.0/8 — RFC 1918
        || a == 127                         // 127.0.0.0/8 — loopback
        || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10 — CGNAT (incl. Alibaba metadata 100.100.100.200)
        || (a == 169 && b == 254)           // 169.254.0.0/16 — link-local (AWS/GCP/Azure metadata)
        || (a == 172 && (16..=31).contains(&b)) // 172.16.0.0/12 — RFC 1918
        || (a == 192 && b == 0 && c == 0)   // 192.0.0.0/24 — IETF protocol assignments
        || (a == 192 && b == 0 && c == 2)   // 192.0.2.0/24 — TEST-NET-1
        || (a == 192 && b == 168)           // 192.168.0.0/16 — RFC 1918
        || (a == 198 && (b == 18 || b == 19)) // 198.18.0.0/15 — benchmarking
        || (a == 198 && b == 51 && c == 100) // 198.51.100.0/24 — TEST-NET-2
        || (a == 203 && b == 0 && c == 113) // 203.0.113.0/24 — TEST-NET-3
        || a >= 224 // 224.0.0.0/4 multicast, 240.0.0.0/4 reserved, 255.255.255.255 broadcast
}

/// The IPv4 address a transition-mechanism IPv6 address carries, if any:
/// IPv4-mapped `::ffff:a.b.c.d`, IPv4-compatible `::a.b.c.d`, NAT64
/// `64:ff9b::a.b.c.d` and 6to4 `2002:aabb:ccdd::`. Each of these can deliver a
/// packet to that IPv4 host, so it is judged as that host.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = ip.segments();
    let from =
        |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if s[0..5] == [0, 0, 0, 0, 0] && (s[5] == 0xffff || s[5] == 0) {
        return Some(from(s[6], s[7]));
    }
    if s[0..6] == [0x0064, 0xff9b, 0, 0, 0, 0] {
        return Some(from(s[6], s[7]));
    }
    if s[0] == 0x2002 {
        return Some(from(s[1], s[2]));
    }
    None
}

/// Check if an IPv6 address is loopback, link-local, or otherwise internal.
fn is_internal_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    if let Some(v4) = embedded_ipv4(ip) {
        return is_internal_ipv4(v4);
    }
    let s = ip.segments();
    (s[0] & 0xffc0) == 0xfe80          // fe80::/10 — link-local
        || (s[0] & 0xffc0) == 0xfec0   // fec0::/10 — deprecated site-local
        || (s[0] & 0xfe00) == 0xfc00   // fc00::/7 — unique local (RFC 4193), incl. AWS fd00:ec2::254
        || (s[0] & 0xff00) == 0xff00   // ff00::/8 — multicast
        || (s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 0x0001) // 64:ff9b:1::/48 — local-use NAT64
        || (s[0] == 0x2001 && s[1] == 0x0000) // 2001::/32 — Teredo (embedded IPv4 is obfuscated)
        || (s[0] == 0x2001 && s[1] == 0x0db8) // 2001:db8::/32 — documentation
        || (s[0] == 0x0100 && s[1..4] == [0, 0, 0]) // 100::/64 — discard-only
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

/// Is this URL pointed at the local machine itself — loopback host, any port?
///
/// Deliberately narrower than [`is_internal_parsed_url`]: it admits 127.0.0.0/8,
/// `::1` and `localhost`, and nothing else. A private LAN address (192.168.x.x)
/// or a cloud metadata endpoint (169.254.169.254) is still internal and still
/// blocked — those are the SSRF targets; the user's own machine is not.
pub(crate) fn is_loopback_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    // Credentials in the URL are the classic disguise for exactly this check
    // (`http://api.openai.com@127.0.0.1/`), so a URL carrying them is never
    // treated as a trusted local endpoint.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    match parsed.host() {
        Some(Host::Ipv4(v4)) => v4.is_loopback(),
        Some(Host::Ipv6(v6)) => v6.is_loopback(),
        Some(Host::Domain(d)) => {
            let host = d.trim_end_matches('.').to_lowercase();
            if host == "localhost" || host.ends_with(".localhost") {
                return true;
            }
            // A bare literal the parser left as a domain may still be an IP.
            matches!(host.parse::<IpAddr>(), Ok(ip) if ip.is_loopback())
        }
        None => false,
    }
}

/// Validate a user-configured LLM endpoint.
///
/// Replaces the provider-NAME check that used to guard these call sites
/// (`if provider != "ollama" { validate_not_internal(url) }`). That was wrong in
/// both directions:
///
///  * as a bypass — `provider: "ollama"` is a value the frontend can set over
///    IPC, and it disabled SSRF validation for ANY `base_url`, including cloud
///    metadata endpoints;
///  * as a false negative (INV-032) — a local LM Studio, llama.cpp or Jan server
///    is exactly as safe as Ollama and was blocked purely for not being called
///    "ollama".
///
/// Keying on the HOST instead fixes both: any loopback endpoint is allowed
/// whatever the provider claims to be, and everything else is validated.
pub(crate) fn validate_llm_endpoint(url: &str) -> Result<()> {
    if is_loopback_url(url) {
        return Ok(());
    }
    validate_not_internal(url)
}

/// Lower-cased host of a URL (root label trimmed, IPv6 without brackets), or
/// `None` when the URL does not parse or has no host. The identity a stored
/// credential is bound to: see `settings_commands::llm_key_destination` and
/// `digest_email::merge_smtp_update`.
pub(crate) fn url_host(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    Some(match parsed.host()? {
        Host::Domain(d) => d.trim_end_matches('.').to_lowercase(),
        Host::Ipv4(v4) => v4.to_string(),
        Host::Ipv6(v6) => v6.to_string(),
    })
}

/// Is this bare host name (no scheme or port) the local machine itself?
pub(crate) fn is_loopback_host(host: &str) -> bool {
    let h = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_lowercase();
    h == "localhost"
        || h.ends_with(".localhost")
        || matches!(h.parse::<IpAddr>(), Ok(ip) if ip.is_loopback())
}

/// Drop every internal address from a DNS answer.
fn public_addrs(addrs: impl IntoIterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    addrs
        .into_iter()
        .filter(|a| !is_internal_ip(a.ip()))
        .collect()
}

/// DNS resolver for the client that fetches user-supplied URLs
/// (`http_client::HTTP_CLIENT`). It resolves through the OS exactly as
/// reqwest's default resolver does, then removes every internal address before
/// a connection can be attempted.
///
/// Why this exists: `validate_not_internal` resolves the host once, and reqwest
/// used to resolve it AGAIN to connect. A hostile DNS server can answer the
/// first lookup with a public address and the second with 127.0.0.1 or
/// 169.254.169.254 (DNS rebinding), so the pre-flight check passed and the
/// request still landed inside. With this resolver the addresses a connection
/// may use are the checked addresses — the check and the connect are the same
/// lookup. A name with only internal addresses fails to connect. IP-literal
/// hosts never reach a resolver; `validate_not_internal` and the redirect
/// policy judge those statically.
pub(crate) struct PublicOnlyResolver;

impl reqwest::dns::Resolve for PublicOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let lookup = host.clone();
            let answer = tokio::task::spawn_blocking(move || {
                (lookup.as_str(), 0u16)
                    .to_socket_addrs()
                    .map(Iterator::collect::<Vec<_>>)
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })??;
            let public = public_addrs(answer);
            if public.is_empty() {
                tracing::warn!(
                    target: "4da::security",
                    host = %host,
                    "Refused connection: host resolves only to internal addresses (SSRF / DNS rebinding)"
                );
                return Err(format!("{host}: {INTERNAL_URL_BLOCKED}").into());
            }
            let addrs: reqwest::dns::Addrs = Box::new(public.into_iter());
            Ok(addrs)
        })
    }
}

#[cfg(test)]
#[path = "url_validation_tests.rs"]
mod hardening_tests;

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

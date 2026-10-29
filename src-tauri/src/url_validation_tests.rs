// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! SSRF hardening tests (audit 2026-10-12, C12): the special-purpose ranges the
//! guard used to miss, and the resolver that pins the validated addresses into
//! the actual connection (DNS rebinding).

use super::*;
use std::str::FromStr;

fn v4(s: &str) -> Ipv4Addr {
    s.parse().expect("ipv4 literal")
}

fn v6(s: &str) -> Ipv6Addr {
    s.parse().expect("ipv6 literal")
}

#[test]
fn special_purpose_ipv4_ranges_are_internal() {
    for ip in [
        "0.1.2.3",         // 0.0.0.0/8 — reaches the local host on Linux/macOS
        "100.64.0.1",      // CGNAT low edge
        "100.127.255.254", // CGNAT high edge
        "192.0.0.170",     // 192.0.0.0/24 (NAT64 discovery)
        "192.0.2.1",       // TEST-NET-1
        "198.18.0.1",      // benchmarking 198.18/15
        "198.19.255.254",
        "198.51.100.7", // TEST-NET-2
        "203.0.113.9",  // TEST-NET-3
        "224.0.0.251",  // multicast (mDNS)
        "240.0.0.1",    // reserved 240/4
        "255.255.255.255",
    ] {
        assert!(is_internal_ipv4(v4(ip)), "{ip} must be internal");
    }
}

#[test]
fn public_ipv4_neighbours_stay_public() {
    for ip in [
        "1.1.1.1",
        "8.8.8.8",
        "100.63.255.255", // just below CGNAT
        "100.128.0.0",    // just above CGNAT
        "192.0.1.1",
        "198.17.255.255",
        "198.20.0.0",
        "223.255.255.255",
    ] {
        assert!(!is_internal_ipv4(v4(ip)), "{ip} must stay public");
    }
}

#[test]
fn ipv6_forms_that_embed_an_internal_ipv4_are_internal() {
    for ip in [
        "::ffff:127.0.0.1",       // IPv4-mapped
        "::ffff:169.254.169.254", // mapped metadata
        "::127.0.0.1",            // IPv4-compatible
        "64:ff9b::7f00:1",        // NAT64 well-known prefix -> 127.0.0.1
        "64:ff9b::a9fe:a9fe",     // NAT64 -> 169.254.169.254
        "2002:7f00:1::",          // 6to4 -> 127.0.0.1
        "2002:c0a8:101::1",       // 6to4 -> 192.168.1.1
    ] {
        assert!(is_internal_ipv6(v6(ip)), "{ip} must be internal");
    }
    for ip in ["::ffff:8.8.8.8", "64:ff9b::808:808", "2002:808:808::1"] {
        assert!(!is_internal_ipv6(v6(ip)), "{ip} embeds a public IPv4");
    }
}

#[test]
fn special_purpose_ipv6_ranges_are_internal() {
    for ip in [
        "fe80::1",                              // link-local
        "fec0::1",                              // site-local (deprecated)
        "fd00:ec2::254",                        // ULA (AWS metadata)
        "ff02::1",                              // multicast
        "64:ff9b:1::1",                         // local-use NAT64
        "2001:0:4136:e378:8000:63bf:3fff:fdd2", // Teredo
        "2001:db8::1",                          // documentation
        "100::1",                               // discard-only
    ] {
        assert!(is_internal_ipv6(v6(ip)), "{ip} must be internal");
    }
    for ip in [
        "2606:4700:4700::1111",
        "2a00:1450:4001:80b::200e",
        "2001:4860::8888",
    ] {
        assert!(!is_internal_ipv6(v6(ip)), "{ip} must stay public");
    }
}

#[test]
fn url_level_checks_cover_the_new_ranges() {
    assert!(validate_not_internal("http://0.0.0.1/").is_err());
    assert!(validate_not_internal("http://198.18.0.1/feed").is_err());
    assert!(validate_not_internal("http://[64:ff9b::7f00:1]/").is_err());
    assert!(validate_not_internal("http://[::ffff:a9fe:a9fe]/latest").is_err());
    assert!(validate_not_internal("http://[2002:7f00:1::]/").is_err());
}

#[test]
fn url_host_normalises() {
    assert_eq!(
        url_host("https://API.Groq.com./openai/v1").as_deref(),
        Some("api.groq.com")
    );
    assert_eq!(url_host("http://[::1]:11434").as_deref(), Some("::1"));
    assert_eq!(
        url_host("http://127.0.0.1:8080/v1").as_deref(),
        Some("127.0.0.1")
    );
    assert_eq!(url_host("not a url"), None);
}

#[test]
fn loopback_host_detection() {
    for h in [
        "localhost",
        "LOCALHOST.",
        "api.localhost",
        "127.0.0.1",
        "127.9.9.9",
        "::1",
        "[::1]",
    ] {
        assert!(is_loopback_host(h), "{h} is loopback");
    }
    for h in ["192.168.1.10", "localhost.evil.com", "smtp.gmail.com", ""] {
        assert!(!is_loopback_host(h), "{h} is not loopback");
    }
}

#[test]
fn public_addrs_drops_internal_answers() {
    let answer: Vec<SocketAddr> = vec![
        "93.184.216.34:0".parse().expect("addr"),
        "127.0.0.1:0".parse().expect("addr"),
        "[::1]:0".parse().expect("addr"),
        "169.254.169.254:0".parse().expect("addr"),
    ];
    let kept = public_addrs(answer);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].ip().to_string(), "93.184.216.34");
}

/// A name whose DNS answer is internal is refused by the resolver itself — the
/// connect-time lookup, not just the pre-flight one.
#[tokio::test]
async fn resolver_refuses_a_name_that_resolves_internal() {
    use reqwest::dns::Resolve;
    let name = reqwest::dns::Name::from_str("localhost").expect("name");
    let result = PublicOnlyResolver::with_exempt_hosts(vec!["proxy.corp".into()])
        .resolve(name)
        .await;
    let err = result.err().expect("non-exempt localhost must be refused");
    assert!(err.to_string().contains("internal/private"), "{err}");
}

/// Review of #958: reqwest resolves the PROXY's hostname through this
/// resolver, so an env/registry proxy on an internal address was refused and
/// every fetch failed. An exempt (proxy) host resolves unfiltered.
#[tokio::test]
async fn resolver_lets_an_exempt_proxy_host_resolve_internal() {
    use reqwest::dns::Resolve;
    let name = reqwest::dns::Name::from_str("localhost").expect("name");
    let addrs: Vec<SocketAddr> = PublicOnlyResolver::with_exempt_hosts(vec!["LocalHost.".into()])
        .resolve(name)
        .await
        .expect("exempt proxy host must resolve")
        .collect();
    assert!(addrs.iter().any(|a| a.ip().is_loopback()), "{addrs:?}");
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn proxy_hosts_parses_env_and_registry_forms() {
    let hosts = proxy_hosts(&pairs(&[
        ("HTTP_PROXY", "proxy.corp:8080"),
        ("https_proxy", "http://user:pw@Secure.Proxy.Corp.:3128/"),
        ("ALL_PROXY", "socks5://socks.corp:1080"),
        ("all_proxy", ""),
        ("HTTP_PROXY", "   "),
        (
            "ProxyServer",
            "http=reg-http.corp:80;https=reg-https.corp:443",
        ),
        ("NO_PROXY", "nope.corp"),
        ("PATH", "C:\\Windows"),
    ]));
    assert_eq!(
        hosts,
        vec![
            "proxy.corp",
            "secure.proxy.corp",
            "socks.corp",
            "reg-http.corp",
            "reg-https.corp"
        ]
    );
}

#[test]
fn proxy_hosts_skips_ip_literals_and_dedups() {
    let hosts = proxy_hosts(&pairs(&[
        ("HTTP_PROXY", "10.0.0.5:8080"),
        ("HTTPS_PROXY", "http://[fd00::1]:3128"),
        ("ProxyServer", "proxy.corp:8080"),
        ("http_proxy", "http://PROXY.corp:8080"),
    ]));
    assert_eq!(hosts, vec!["proxy.corp"]);
}

/// End to end: a client using a proxy named `localhost` (internal) with that
/// host exempt reaches the proxy; without the exemption it cannot.
#[tokio::test]
async fn proxied_request_reaches_an_exempt_internal_proxy() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf).await;
            let _ = sock
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 7\r\nconnection: close\r\n\r\nproxied",
                )
                .await;
        }
    });
    let client = |exempt: Vec<String>| {
        reqwest::Client::builder()
            .proxy(reqwest::Proxy::all(format!("http://localhost:{port}")).expect("proxy"))
            .dns_resolver(std::sync::Arc::new(PublicOnlyResolver::with_exempt_hosts(
                exempt,
            )))
            .build()
            .expect("client")
    };
    let ok = client(vec!["localhost".into()])
        .get("http://feeds.example.org/rss")
        .send()
        .await
        .expect("request through exempt proxy");
    assert_eq!(ok.text().await.expect("body"), "proxied");
    let refused = client(Vec::new())
        .get("http://feeds.example.org/rss")
        .send()
        .await;
    assert!(
        refused.is_err(),
        "non-exempt internal proxy must be refused"
    );
}

/// The rebinding window, end to end through the client that fetches feeds:
/// a hostname (no pre-flight check at all here) that resolves to loopback must
/// not reach the server listening there. Before the resolver was wired into
/// `HTTP_CLIENT` this request succeeded with HTTP 200.
#[tokio::test]
async fn shared_fetch_client_cannot_connect_to_a_name_resolving_internal() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = sock.read(&mut buf).await;
            let _ = sock
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 6\r\nconnection: close\r\n\r\nsecret",
                )
                .await;
        }
    });

    // Control: without the resolver the same request reaches the server, so a
    // failure below is the resolver's doing, not an unreachable listener.
    let control = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client")
        .get(format!("http://localhost:{port}/"))
        .send()
        .await;
    assert!(control.is_ok(), "control request failed: {control:?}");

    let result = crate::http_client::HTTP_CLIENT
        .get(format!("http://localhost:{port}/"))
        .send()
        .await;
    assert!(
        result.is_err(),
        "request to a name resolving to loopback must fail, got {:?}",
        result.map(|r| r.status())
    );
}

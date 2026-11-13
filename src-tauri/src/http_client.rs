// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared HTTP clients for outbound requests.
//!
//! Every outbound client in the app, pooled or purpose-built, starts from
//! [`outbound_builder`] (or [`outbound_blocking_builder`]): the OS trust
//! store on top of the bundled roots, and `network.proxy_url` when set
//! (audit P5). A test fails on a raw `reqwest::Client::builder()` elsewhere.
//!
//! Three pooled clients with distinct timeout profiles:
//! - `HTTP_CLIENT` — general-purpose, 30s timeout, 10s connect
//! - `PROBE_CLIENT` — health checks & API validation, 15s timeout, 5s connect
//! - `TEAM_CLIENT` — team relay operations, 15s timeout, TeamSync user-agent
//!
//! **When NOT to use these clients (keep purpose-built):**
//! - `embeddings.rs` — needs 90s timeout for large embedding batches
//! - `llm.rs` — needs dynamic timeouts (60s cloud, 120s Ollama cold start)
//! - `settings_commands_llm.rs::test_ollama_connection_impl` — 120s for cold model load
//! - `settings_commands_llm.rs::pull_ollama_model` — 600s for model downloads
//! - `settings_commands_llm.rs::detect_local_servers` — 3s intentionally fast probe
//! - `team_sync_scheduler.rs` — 30s timeout for background sync cycles
//! - `webhooks.rs::deliver_webhook` — 10s timeout for fire-and-forget
//! - `calibration_commands.rs` — 3s quick Ollama check
//! - `sources/mod.rs` — already has its own `SHARED_CLIENT` with identical config

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::redirect::{Action, Attempt, Policy};

/// Redirect hops permitted before a request is abandoned.
/// Matches reqwest's own default (`Policy::limited(10)`), which
/// `Policy::custom` replaces wholesale — a custom policy that forgets to count
/// hops will follow a redirect loop forever.
const MAX_REDIRECTS: usize = 10;

/// Error text used when a redirect hop is refused for targeting an internal
/// address. Asserted on in tests; surfaced through `reqwest::Error::is_redirect`.
pub(crate) const SSRF_REDIRECT_BLOCKED: &str =
    "redirect blocked: hop targets an internal/private network address (SSRF prevention)";

/// Shared decision function for every guarded redirect policy.
///
/// The pre-flight `validate_not_internal` checks only the URL the caller hands
/// us. Without this, one `302 Location: http://127.0.0.1:4446/…` from a hostile
/// RSS feed (or a hijacked curated-feed domain) walks straight past all ten
/// pre-flight call sites, because reqwest's default `Policy::limited(10)`
/// follows the hop and returns the internal body as if it were the feed's.
///
/// `allow_internal_origin` exists for the local-LLM clients: a user who
/// deliberately points 4DA at `http://127.0.0.1:11434` (Ollama, llama-server)
/// must still be able to follow that server's own redirects. It permits an
/// internal hop *only* when the request already started internal, so a cloud
/// provider can never redirect its way inward.
fn decide(attempt: Attempt<'_>, allow_internal_origin: bool) -> Action {
    if attempt.previous().len() >= MAX_REDIRECTS {
        return attempt.error(format!("too many redirects (limit {MAX_REDIRECTS})"));
    }

    if crate::url_validation::is_internal_parsed_url(attempt.url()) {
        let started_internal = attempt
            .previous()
            .first()
            .is_some_and(crate::url_validation::is_internal_parsed_url);

        if !(allow_internal_origin && started_internal) {
            tracing::warn!(
                target: "4da::security",
                hop = %attempt.url(),
                origin = %attempt.previous().first().map_or("<unknown>", url::Url::as_str),
                "Blocked redirect to internal address (SSRF prevention)"
            );
            return attempt.error(SSRF_REDIRECT_BLOCKED);
        }
    }

    attempt.follow()
}

/// Redirect policy for clients that must never reach an internal address.
/// Use for anything fetching remote content or calling a remote API.
pub(crate) fn ssrf_guarded_redirect_policy() -> Policy {
    Policy::custom(|attempt| decide(attempt, false))
}

/// Redirect policy for clients whose endpoint may legitimately be local
/// (Ollama, llama-server, the dev frontend). Internal hops are permitted only
/// when the original request was itself internal.
pub(crate) fn local_aware_redirect_policy() -> Policy {
    Policy::custom(|attempt| decide(attempt, true))
}

/// Global HTTP client with shared connection pool and TLS session cache.
/// Suitable for health checks, license validation, API probes, and other
/// general-purpose requests where the default 30s timeout is appropriate.
///
/// Every source adapter and user-supplied feed/scrape URL goes through this
/// client (`sources::shared_client`), so it connects ONLY to public
/// addresses: `PublicOnlyResolver` pins the connection to the addresses the
/// SSRF check accepts, closing the DNS-rebinding window between
/// `validate_not_internal` and the request. Never point it at localhost —
/// local endpoints (Ollama) use `PROBE_CLIENT` or their own clients.
pub(crate) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    outbound_builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .dns_resolver(std::sync::Arc::new(
            crate::url_validation::PublicOnlyResolver::from_system(),
        ))
        .redirect(ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build HTTP client: {e}, using default");
            default_client()
        })
});

/// Shared client for quick health checks, status probes, and API validation.
/// Tight timeouts prevent blocking on unresponsive services.
pub(crate) static PROBE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    outbound_builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build probe client: {e}, using default");
            default_client()
        })
});

// ============================================================================
// Every outbound client: the OS trust store and the configured proxy (P5)
// ============================================================================

/// `network.proxy_url` as last loaded or saved (`settings::manager`).
static CONFIGURED_PROXY: parking_lot::RwLock<Option<String>> = parking_lot::RwLock::new(None);
/// Whether settings were loaded in this process ([`publish_proxy`] ran).
static PROXY_PUBLISHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Mirror `network.proxy_url` for lock-free reads. Called whenever settings
/// are loaded or saved. Clients already built keep the proxy they were built
/// with; the shared pooled clients are built once, so a change takes effect
/// on the next start.
pub(crate) fn publish_proxy(url: Option<&str>) {
    let url = url
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string);
    *CONFIGURED_PROXY.write() = url;
    PROXY_PUBLISHED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// The configured proxy URL, if any. Before settings have been published,
/// reads them without initializing them. If they are being written at that
/// moment, it waits for them; if it still cannot read them, requests are
/// refused ([`REFUSE_PROXY`]) rather than sent without a proxy that may be
/// configured.
pub(crate) fn configured_proxy() -> Option<String> {
    if PROXY_PUBLISHED.load(std::sync::atomic::Ordering::SeqCst) {
        return CONFIGURED_PROXY.read().clone();
    }
    let manager = crate::state::try_get_settings_manager()?;
    match manager.try_lock_for(Duration::from_secs(5)) {
        Some(guard) => guard
            .get()
            .network
            .proxy_url
            .clone()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty()),
        None => {
            tracing::error!(
                target: "4da::http",
                "settings could not be read to find network.proxy_url; this client refuses requests"
            );
            Some(REFUSE_PROXY.to_string())
        }
    }
}

/// Where requests go when `network.proxy_url` cannot be used: a closed
/// loopback port, so they fail instead of silently leaving without the
/// proxy the user asked for (a Tor user's `socks5://` URL in a build without
/// SOCKS support must not fall back to a direct connection).
const REFUSE_PROXY: &str = "http://127.0.0.1:1";

/// `url` when this build can use it as a proxy, else [`REFUSE_PROXY`].
/// Only `http://` and `https://` proxies with a host qualify: hyper-util
/// silently ignores any other scheme (`socks://`, a typo such as `htpp://`,
/// `ftp://`), and because a configured proxy also turns the system proxy
/// off, such a URL used to send every request out directly. SOCKS support
/// is not compiled in.
pub(crate) fn usable_proxy_url(url: &str) -> &str {
    let usable = url::Url::parse(url).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https") && u.host_str().is_some_and(|h| !h.is_empty())
    }) && reqwest::Proxy::all(url).is_ok();
    if usable {
        url
    } else {
        tracing::error!(
            target: "4da::http",
            "network.proxy_url is not an http:// or https:// proxy; outbound requests are refused until it is fixed"
        );
        REFUSE_PROXY
    }
}

/// Destinations that never go through the configured proxy: this machine
/// and the private networks (a LAN Ollama, a local llama-server, a NAS):
/// loopback, RFC 1918, link-local, CGNAT (Tailscale), unique-local IPv6,
/// `localhost`, `*.local` and single-label host names.
const LOCAL_NETS: &[&str] = &[
    "127.0.0.0/8",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "169.254.0.0/16",
    "100.64.0.0/10",
    "::1/128",
    "fc00::/7",
    "fe80::/10",
];

/// Whether `ip` is inside the CIDR block `net` (`a.b.c.d/n` or `x::/n`).
fn in_cidr(ip: std::net::IpAddr, net: &str) -> bool {
    let Some((base, bits)) = net.split_once('/') else {
        return base_matches(ip, net);
    };
    let (Ok(base), Ok(bits)) = (base.parse::<std::net::IpAddr>(), bits.parse::<u32>()) else {
        return false;
    };
    match (ip, base) {
        (std::net::IpAddr::V4(a), std::net::IpAddr::V4(b)) if bits <= 32 => {
            let mask = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - bits)
            };
            u32::from(a) & mask == u32::from(b) & mask
        }
        (std::net::IpAddr::V6(a), std::net::IpAddr::V6(b)) if bits <= 128 => {
            let mask = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - bits)
            };
            u128::from(a) & mask == u128::from(b) & mask
        }
        _ => false,
    }
}

fn base_matches(ip: std::net::IpAddr, entry: &str) -> bool {
    entry.parse::<std::net::IpAddr>().is_ok_and(|e| e == ip)
}

/// Whether a request to `host` bypasses the configured proxy: a local
/// destination, or a `NO_PROXY` entry (host, `.domain`, IP or CIDR). A bare
/// `*` in `NO_PROXY` is ignored: it would switch off the proxy the user set
/// in 4DA's own settings.
pub(crate) fn bypasses_proxy(host: &str, env_no_proxy: &str) -> bool {
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return LOCAL_NETS.iter().any(|n| in_cidr(ip, n))
            || env_no_proxy
                .split(',')
                .map(str::trim)
                .filter(|e| !e.is_empty() && *e != "*")
                .any(|e| in_cidr(ip, e));
    }
    let last_label = host.rsplit('.').next().unwrap_or("");
    if last_label == "localhost" || (last_label == "local" && host.contains('.')) {
        return true;
    }
    if !host.contains('.') {
        return true;
    }
    env_no_proxy
        .split(',')
        .map(|e| e.trim().trim_start_matches("*.").trim_start_matches('.'))
        .map(str::to_ascii_lowercase)
        .filter(|e| {
            !e.is_empty() && e != "*" && e.parse::<std::net::IpAddr>().is_err() && !e.contains('/')
        })
        .any(|e| host == e || host.ends_with(&format!(".{e}")))
}

/// The proxy for `url`, bypassed per [`bypasses_proxy`]. An unusable URL
/// refuses ([`usable_proxy_url`]).
pub(crate) fn proxy_for(url: &str) -> reqwest::Proxy {
    let env_no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();
    let target = usable_proxy_url(url).to_string();
    reqwest::Proxy::custom(move |dest| {
        let host = dest.host_str().unwrap_or("");
        if bypasses_proxy(host, &env_no_proxy) {
            None
        } else {
            Some(target.clone())
        }
    })
}

/// The builder every outbound async client starts from: certificates from
/// the operating system's trust store (so a corporate TLS-inspection root
/// works) on top of the bundled Mozilla roots, and `network.proxy_url` when
/// set. The system proxy (`HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY`,
/// `NO_PROXY`, and on Windows the Internet Options proxy) applies as before.
pub(crate) fn outbound_builder() -> reqwest::ClientBuilder {
    builder_with(configured_proxy().as_deref())
}

/// [`outbound_builder`] with the proxy supplied (the test seam).
pub(crate) fn builder_with(proxy: Option<&str>) -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder().tls_built_in_native_certs(true);
    match proxy {
        Some(url) => builder.proxy(proxy_for(url)),
        None => builder,
    }
}

/// The fallback when a configured client fails to build: still the trust
/// store and the proxy, only without the caller's timeouts and policies.
pub(crate) fn default_client() -> reqwest::Client {
    outbound_builder().build().unwrap_or_default()
}

/// [`outbound_builder`] for the blocking client (model downloads).
pub(crate) fn outbound_blocking_builder() -> reqwest::blocking::ClientBuilder {
    let builder = reqwest::blocking::Client::builder().tls_built_in_native_certs(true);
    match configured_proxy() {
        Some(url) => builder.proxy(proxy_for(&url)),
        None => builder,
    }
}

/// Shared client for team relay operations (sync, create, join).
/// Uses team-specific user-agent for relay identification. Only the
/// `team-sync` build calls it (`team_sync_commands/lifecycle.rs`).
#[cfg_attr(not(feature = "team-sync"), allow(dead_code))]
pub(crate) static TEAM_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    outbound_builder()
        .user_agent("4DA-TeamSync/1.0")
        .timeout(Duration::from_secs(15))
        .redirect(ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build team client: {e}, using default");
            default_client()
        })
});

#[cfg(test)]
mod redirect_policy_tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Body only reachable by *following* the redirect. If a test sees this
    /// string, the hop happened.
    const INTERNAL_BODY: &str = "SECRET-INTERNAL-BODY";

    /// Minimal HTTP/1.1 fixture bound to loopback.
    ///
    /// This is a real socket serving a real `302`, not a mocked policy: the
    /// only way to exercise `redirect::Policy` is through a live redirect,
    /// because `reqwest::redirect::Attempt` cannot be constructed outside the
    /// crate.
    ///
    /// - `/redirect-internal` → `302` to `/internal` on this same loopback port
    /// - `/loop`              → `302` to itself (redirect-loop bounding)
    /// - anything else        → `200` with `INTERNAL_BODY`
    async fn spawn_fixture() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback fixture");
        let addr = listener.local_addr().expect("fixture local addr");

        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(n) = sock.read(&mut buf).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buf[..n]);
                    let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();

                    let response = match path.as_str() {
                        "/redirect-internal" => format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/internal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            addr.port()
                        ),
                        "/loop" => format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/loop\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            addr.port()
                        ),
                        _ => format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{INTERNAL_BODY}",
                            INTERNAL_BODY.len()
                        ),
                    };

                    let _ = sock.write_all(response.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });

        addr
    }

    /// A client with reqwest's stock configuration — i.e. every client in this
    /// codebase before this change.
    fn unguarded_client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("build unguarded client")
    }

    fn guarded_client(policy: Policy) -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .redirect(policy)
            .build()
            .expect("build guarded client")
    }

    /// Baseline: proves the fixture actually redirects and that the stock
    /// reqwest policy (`Policy::limited(10)`) follows the hop and hands back
    /// the internal body. This is the defect, reproduced.
    #[tokio::test]
    async fn stock_reqwest_policy_follows_redirect_to_loopback() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/redirect-internal", addr.port());

        let body = unguarded_client()
            .get(&url)
            .send()
            .await
            .expect("stock client follows the hop")
            .text()
            .await
            .expect("read body");

        assert_eq!(
            body, INTERNAL_BODY,
            "fixture must actually redirect — otherwise the guard test below proves nothing"
        );
    }

    /// The fix: the same fixture, the same hop, refused.
    #[tokio::test]
    async fn guarded_policy_refuses_redirect_to_loopback() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/redirect-internal", addr.port());

        let err = guarded_client(ssrf_guarded_redirect_policy())
            .get(&url)
            .send()
            .await
            .expect_err("redirect to loopback must be refused");

        assert!(err.is_redirect(), "expected a redirect error, got: {err}");
        let chain = format!("{err:?}");
        assert!(
            chain.contains(SSRF_REDIRECT_BLOCKED),
            "error should carry the SSRF reason, got: {chain}"
        );
    }

    /// The shared client every source, scraper, and enrichment fetch uses.
    #[tokio::test]
    async fn shared_http_client_refuses_redirect_to_loopback() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/redirect-internal", addr.port());

        let result = HTTP_CLIENT.get(&url).send().await;

        assert!(
            result.as_ref().is_err_and(reqwest::Error::is_redirect),
            "HTTP_CLIENT followed the hop: {:?}",
            result.map(|r| r.status())
        );
    }

    /// PROBE_CLIENT and TEAM_CLIENT share the same policy.
    #[tokio::test]
    async fn probe_and_team_clients_refuse_redirect_to_loopback() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/redirect-internal", addr.port());

        assert!(
            PROBE_CLIENT
                .get(&url)
                .send()
                .await
                .is_err_and(|e| e.is_redirect()),
            "PROBE_CLIENT followed the hop"
        );
        assert!(
            TEAM_CLIENT
                .get(&url)
                .send()
                .await
                .is_err_and(|e| e.is_redirect()),
            "TEAM_CLIENT followed the hop"
        );
    }

    /// No false positives: a plain 200 from the same loopback fixture is not a
    /// redirect, so the policy never runs and the request succeeds.
    #[tokio::test]
    async fn guarded_policy_leaves_non_redirects_alone() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/plain", addr.port());

        let body = guarded_client(ssrf_guarded_redirect_policy())
            .get(&url)
            .send()
            .await
            .expect("non-redirect request should succeed")
            .text()
            .await
            .expect("read body");

        assert_eq!(body, INTERNAL_BODY);
    }

    /// The local-LLM exemption: a request that *started* internal (Ollama at
    /// 127.0.0.1:11434) may follow that server's own redirects.
    #[tokio::test]
    async fn local_aware_policy_allows_internal_to_internal_hop() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/redirect-internal", addr.port());

        let body = guarded_client(local_aware_redirect_policy())
            .get(&url)
            .send()
            .await
            .expect("internal→internal hop should be permitted")
            .text()
            .await
            .expect("read body");

        assert_eq!(body, INTERNAL_BODY);
    }

    /// `Policy::custom` replaces reqwest's hop limit entirely. Without an
    /// explicit counter this request never terminates.
    #[tokio::test]
    async fn custom_policy_still_bounds_redirect_loops() {
        let addr = spawn_fixture().await;
        let url = format!("http://127.0.0.1:{}/loop", addr.port());

        let err = tokio::time::timeout(
            Duration::from_secs(20),
            guarded_client(local_aware_redirect_policy())
                .get(&url)
                .send(),
        )
        .await
        .expect("redirect loop must terminate, not hang")
        .expect_err("redirect loop must error");

        assert!(err.is_redirect(), "expected a redirect error, got: {err}");
        assert!(
            format!("{err:?}").contains("too many redirects"),
            "expected the hop-limit message, got: {err:?}"
        );
    }
}

#[cfg(test)]
mod outbound_tests {
    use super::*;
    use std::path::Path;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A loopback server that answers every request with `body` and records
    /// each request line (an HTTP proxy receives the absolute URL there).
    async fn recording_server(
        body: &'static str,
    ) -> (u16, std::sync::Arc<parking_lot::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let seen = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let log = log.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(n) = sock.read(&mut buf).await else {
                        return;
                    };
                    let request = String::from_utf8_lossy(&buf[..n]);
                    log.lock()
                        .push(request.lines().next().unwrap_or("").to_string());
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(response.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        (port, seen)
    }

    /// P5: `network.proxy_url` was read by a function nothing called, so a
    /// configured proxy was ignored. The builder every client starts from
    /// now routes through it.
    #[tokio::test]
    async fn the_configured_proxy_carries_outbound_requests() {
        let (port, seen) = recording_server("via-proxy").await;
        let client = builder_with(Some(&format!("http://127.0.0.1:{port}")))
            .timeout(Duration::from_secs(5))
            .build()
            .expect("client");

        let body = client
            .get("http://registry.example.invalid/react")
            .send()
            .await
            .expect("through the proxy")
            .text()
            .await
            .expect("body");

        assert_eq!(body, "via-proxy");
        assert_eq!(
            seen.lock().as_slice(),
            ["GET http://registry.example.invalid/react HTTP/1.1"]
        );
    }

    /// This machine and the private ranges never go through the proxy (a
    /// local Ollama keeps working behind a corporate proxy).
    #[tokio::test]
    async fn local_addresses_bypass_the_configured_proxy() {
        let (proxy_port, proxied) = recording_server("via-proxy").await;
        let (port, _) = recording_server("direct").await;
        let client = builder_with(Some(&format!("http://127.0.0.1:{proxy_port}")))
            .timeout(Duration::from_secs(5))
            .build()
            .expect("client");

        let body = client
            .get(format!("http://127.0.0.1:{port}/api/version"))
            .send()
            .await
            .expect("direct")
            .text()
            .await
            .expect("body");

        assert_eq!(body, "direct");
        assert!(
            proxied.lock().is_empty(),
            "the proxy saw {:?}",
            proxied.lock()
        );
    }

    /// A proxy URL this build cannot use refuses requests instead of
    /// letting them leave without the proxy the user asked for. Review of
    /// #1130: hyper-util drops any scheme but http/https/socks and a set proxy
    /// turns the system proxy off, so `socks://`, `htpp://` or `ftp://`
    /// passed the old parse-only check and sent everything direct.
    #[test]
    fn an_unusable_proxy_url_refuses_rather_than_going_direct() {
        for good in [
            "http://proxy.corp.example:8080",
            "https://user:pw@proxy.corp.example",
        ] {
            assert_eq!(usable_proxy_url(good), good);
        }
        for bad in [
            "::not a proxy url",
            "socks://127.0.0.1:1080",
            "socks5://127.0.0.1:9050",
            "htpp://proxy.corp:8080",
            "ftp://proxy.corp:21",
            "http://",
        ] {
            assert_eq!(usable_proxy_url(bad), REFUSE_PROXY, "{bad}");
        }
        assert!(builder_with(Some("::not a proxy url")).build().is_ok());
    }

    /// Local destinations bypass the configured proxy; `NO_PROXY=*` does not
    /// switch off a proxy set in 4DA's own settings.
    #[test]
    fn local_destinations_and_no_proxy_entries_bypass_the_proxy() {
        for local in [
            "localhost",
            "127.0.0.1",
            "[::1]",
            "192.168.1.20",
            "10.1.2.3",
            "100.101.102.103",
            "fd12:3456::1",
            "ollama-box",
            "nas.local",
        ] {
            assert!(bypasses_proxy(local, ""), "{local}");
        }
        for remote in ["registry.npmjs.org", "8.8.8.8", "2606:4700::1111"] {
            assert!(!bypasses_proxy(remote, ""), "{remote}");
            assert!(
                !bypasses_proxy(remote, "*"),
                "NO_PROXY=* must not bypass {remote}"
            );
        }
        assert!(bypasses_proxy("git.corp.example", ".corp.example"));
        assert!(bypasses_proxy("corp.example", "corp.example"));
        assert!(bypasses_proxy("203.0.113.9", "203.0.113.0/24"));
        assert!(!bypasses_proxy("notcorp.example", "corp.example"));
    }

    /// Blank every `#[cfg(test)]` item (a `mod tests { ... }`, a test-only fn
    /// or `use`), keeping line numbers.
    fn strip_test_items(text: &str) -> String {
        let mut out: Vec<char> = text.chars().collect();
        let marker: Vec<char> = "#[cfg(test)]".chars().collect();
        let mut i = 0;
        while i + marker.len() <= out.len() {
            if out[i..i + marker.len()] != marker[..] {
                i += 1;
                continue;
            }
            let start = i;
            let mut j = i + marker.len();
            while j < out.len() && out[j] != '{' && out[j] != ';' {
                j += 1;
            }
            let mut end = j;
            if j < out.len() && out[j] == '{' {
                let mut depth = 0i32;
                while end < out.len() {
                    match out[end] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    end += 1;
                }
            }
            for c in out.iter_mut().take(end + 1).skip(start) {
                if *c != '\n' {
                    *c = ' ';
                }
            }
            i = end + 1;
        }
        out.into_iter().collect()
    }

    #[test]
    fn the_guard_sees_production_code_after_a_test_item() {
        let text = "#[cfg(test)]\nfn helper() { let _ = reqwest::Client::new(); }\nfn real() { let c = Client::builder(); }\n";
        let stripped = strip_test_items(text);
        assert!(!stripped.contains("reqwest::Client::new()"));
        assert!(stripped.contains("Client::builder()"));
    }

    /// P5: every client used to be built with `reqwest::Client::builder()`
    /// at its call site (bundled roots only, no configured proxy). Production
    /// code now builds them from `outbound_builder` / `outbound_blocking_builder`
    /// only; a new raw builder (also through an import alias:
    /// `Client::builder()`, `ClientBuilder::new()`) fails here. Every
    /// production line of every file is read; only `#[cfg(test)]` items are
    /// skipped.
    #[test]
    fn every_production_client_starts_from_outbound_builder() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let raw = ["Client::builder()", "Client::new()", "ClientBuilder::new()"];
        let mut offenders = Vec::new();
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read src").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let rel = path
                    .strip_prefix(&src)
                    .expect("under src")
                    .to_string_lossy()
                    .replace('\\', "/");
                if !rel.ends_with(".rs") || rel.ends_with("_tests.rs") || rel == "http_client.rs" {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("read");
                let code = strip_test_items(&text);
                for (i, line) in code.lines().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    if raw.iter().any(|r| line.contains(r)) {
                        offenders.push(format!("{rel}:{}", i + 1));
                    }
                }
            }
        }
        offenders.sort();
        assert!(
            offenders.is_empty(),
            "build outbound clients from http_client::outbound_builder: {offenders:?}"
        );
    }
}

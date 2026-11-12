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
/// reads them once without initializing them (never blocks on the lock).
pub(crate) fn configured_proxy() -> Option<String> {
    if PROXY_PUBLISHED.load(std::sync::atomic::Ordering::SeqCst) {
        return CONFIGURED_PROXY.read().clone();
    }
    crate::state::try_get_settings_manager()
        .and_then(|m| m.try_lock())
        .and_then(|guard| guard.get().network.proxy_url.clone())
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
}

/// Destinations that never go through the configured proxy: this machine
/// and the private network ranges (a LAN Ollama, a local llama-server).
const LOCAL_NO_PROXY: &str =
    "localhost,127.0.0.1,::1,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,169.254.0.0/16";

/// Where requests go when `network.proxy_url` cannot be used: a closed
/// loopback port, so they fail instead of silently leaving without the
/// proxy the user asked for (a Tor user's `socks5://` URL in a build without
/// SOCKS support must not fall back to a direct connection).
const REFUSE_PROXY: &str = "http://127.0.0.1:1";

/// `url` when this build can use it as a proxy, else [`REFUSE_PROXY`].
pub(crate) fn usable_proxy_url(url: &str) -> &str {
    match reqwest::Proxy::all(url) {
        Ok(_) => url,
        Err(e) => {
            tracing::error!(
                target: "4da::http",
                error = %e,
                "network.proxy_url is not a proxy this build can use; outbound requests are refused until it is fixed"
            );
            REFUSE_PROXY
        }
    }
}

/// The proxy for `url`, bypassed for [`LOCAL_NO_PROXY`] and the hosts in
/// `NO_PROXY` / `no_proxy`. An unusable URL refuses ([`usable_proxy_url`]).
pub(crate) fn proxy_for(url: &str) -> reqwest::Proxy {
    let env_no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();
    let no_proxy = reqwest::NoProxy::from_string(&format!("{LOCAL_NO_PROXY},{env_no_proxy}"));
    match reqwest::Proxy::all(usable_proxy_url(url)) {
        Ok(proxy) => proxy.no_proxy(no_proxy),
        // REFUSE_PROXY is a constant that parses; never fall back to direct.
        Err(_) => reqwest::Proxy::custom(|_| Some(REFUSE_PROXY)),
    }
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
    /// letting them leave without the proxy the user asked for.
    #[test]
    fn an_unusable_proxy_url_refuses_rather_than_going_direct() {
        assert_eq!(
            usable_proxy_url("http://proxy.corp.example:8080"),
            "http://proxy.corp.example:8080"
        );
        assert_eq!(usable_proxy_url("::not a proxy url"), REFUSE_PROXY);
        assert!(builder_with(Some("::not a proxy url")).build().is_ok());
    }

    /// P5: every client used to be built with `reqwest::Client::builder()`
    /// at its call site (bundled roots only, no configured proxy). Production
    /// code now builds them from `outbound_builder` / `outbound_blocking_builder`
    /// only; a new raw builder fails here.
    #[test]
    fn every_production_client_starts_from_outbound_builder() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let raw = [
            "reqwest::Client::builder()",
            "reqwest::Client::new()",
            "reqwest::blocking::Client::builder()",
            "reqwest::blocking::Client::new()",
        ];
        let test_mod =
            regex::Regex::new(r"#\[cfg\(test\)\]\s*\n\s*(pub(\(crate\)|\(super\))? )?mod ")
                .expect("regex");
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
                let code = match test_mod.find(&text) {
                    Some(m) => &text[..m.start()],
                    None => text.as_str(),
                };
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

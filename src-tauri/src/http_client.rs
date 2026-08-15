// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared HTTP clients for outbound requests.
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
pub(crate) static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build HTTP client: {e}, using default");
            reqwest::Client::new()
        })
});

/// Shared client for quick health checks, status probes, and API validation.
/// Tight timeouts prevent blocking on unresponsive services.
pub(crate) static PROBE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build probe client: {e}, using default");
            reqwest::Client::new()
        })
});

/// Create a reqwest client builder with optional proxy from settings.
/// Call this when you need a client that respects the user's proxy config.
#[allow(dead_code)]
pub(crate) fn client_builder_with_proxy() -> reqwest::ClientBuilder {
    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; desktop-app)")
        .redirect(ssrf_guarded_redirect_policy());

    // Try to read proxy from settings
    let manager = crate::get_settings_manager();
    if let Some(guard) = manager.try_lock() {
        if let Some(ref proxy_url) = guard.get().network.proxy_url {
            if !proxy_url.trim().is_empty() {
                match reqwest::Proxy::all(proxy_url) {
                    Ok(proxy) => {
                        tracing::info!(target: "4da::http", proxy = %proxy_url, "Proxy configured");
                        builder = builder.proxy(proxy);
                    }
                    Err(e) => {
                        tracing::warn!(target: "4da::http", error = %e, proxy = %proxy_url, "Invalid proxy URL — ignoring");
                    }
                }
            }
        }
    }

    builder
}

/// Shared client for team relay operations (sync, create, join).
/// Uses team-specific user-agent for relay identification.
#[allow(dead_code)]
pub(crate) static TEAM_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent("4DA-TeamSync/1.0")
        .timeout(Duration::from_secs(15))
        .redirect(ssrf_guarded_redirect_policy())
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to build team client: {e}, using default");
            reqwest::Client::new()
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

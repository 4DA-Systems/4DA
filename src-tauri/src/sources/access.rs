// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Source access resilience — decouple a *source* (an intent, e.g. "Reddit discussion") from the
//! *access strategy* used to reach it (official JSON API, RSS bridge, an open-protocol mirror, a
//! user-credentialed endpoint, ...).
//!
//! ## Why this exists
//! The open web is being enclosed: Reddit, X and Stack Overflow are walling off the high-signal
//! community data 4DA reads, specifically because AI made it valuable. A source adapter that hardcodes
//! ONE access path dies the moment that path is throttled — which is exactly what happened to Reddit
//! (its `.json` API now times out while its `.rss` endpoint still answers HTTP 200).
//!
//! The durable answer is not any single endpoint — it is a **failover architecture**: every source
//! declares an *ordered list* of access strategies, and [`resilient_fetch`] tries them in turn,
//! returning the first that produces data and, on total failure, the **most actionable** error so the
//! caller can tell "needs a credential" apart from "transient network blip".
//!
//! ## North star — trust-model diversity
//! No single access-failure mode may ever be load-bearing. RSS is still the *same gatekeeper* and can
//! be killed too; even open protocols (Bluesky/AT, Mastodon/ActivityPub) could enclose later. So the
//! strategy list for a mature source should span *different trust models* (corporate API, RSS bridge,
//! open-federated mirror, user-credentialed), and 4DA's local-first model is the structural edge:
//! each user fetches from their own IP at personal volume with their own (free-tier) credential —
//! distributed and low-volume where a centralized crawler is a single blockable, payable choke point.
//!
//! ## Scope (deliberately minimal — not a framework)
//! v1 is "first strategy that yields any items wins". Merging partial results across strategies, a
//! cost/budget model, and per-strategy health persistence are future increments, intentionally not
//! built yet.

use async_trait::async_trait;

use super::{SourceError, SourceItem, SourceResult};

/// One concrete way to reach a source's content. Adapters compose an ordered `Vec<Box<dyn
/// AccessStrategy>>` (preferred first) and hand it to [`resilient_fetch`].
#[async_trait]
pub trait AccessStrategy: Send + Sync {
    /// Stable short label for telemetry / health, e.g. `"reddit:json"`, `"reddit:rss"`.
    fn label(&self) -> &str;

    /// Whether this strategy is inert without a user-supplied credential. The credential-free base
    /// (public API, RSS, open-protocol) must come first so the product works with zero keys;
    /// credentialed strategies are opt-in *depth*, included only when a key is present.
    fn requires_credential(&self) -> bool {
        false
    }

    /// Attempt to fetch items via this strategy. Returning `Ok(vec![])` means "reached the source,
    /// it had nothing" (a real signal); returning `Err` means "this path failed, try the next".
    async fn fetch(&self) -> SourceResult<Vec<SourceItem>>;
}

/// Actionability rank for choosing which error to surface when EVERY strategy failed. Higher = more
/// useful to the caller/user. `Forbidden` wins because it is the one a human can act on (provide a
/// credential / enable the source); transient classes rank below it. Also reused by adapters that
/// aggregate across sub-fetches (e.g. Reddit across subreddits) to pick a representative failure.
pub(crate) fn actionability(e: &SourceError) -> u8 {
    match e {
        SourceError::Forbidden(_) => 5,
        SourceError::RateLimited(_) => 4,
        SourceError::Parse(_) => 3,
        SourceError::Network(_) => 2,
        SourceError::Other(_) => 1,
        SourceError::Disabled => 0,
    }
}

/// Try each access strategy in order; return the first that yields a non-empty result. Failover
/// rules:
/// - `Ok(items)` with items → return immediately (record the winning strategy).
/// - `Ok(vec![])` → the source was reachable but empty; remember it and keep trying for data, but if
///   nothing better turns up, return the empty success (an honest "nothing right now").
/// - `Err(_)` → log and advance to the next strategy.
///
/// If no strategy yielded any `Ok`, return the most *actionable* error across all failures so the
/// caller can surface a credential prompt instead of a generic "network error".
pub async fn resilient_fetch(
    source_type: &str,
    strategies: &[Box<dyn AccessStrategy>],
) -> SourceResult<Vec<SourceItem>> {
    let mut errors: Vec<SourceError> = Vec::new();
    let mut reached_but_empty = false;

    for strategy in strategies {
        match strategy.fetch().await {
            Ok(items) if !items.is_empty() => {
                tracing::info!(
                    target: "4da::sources::access",
                    source = source_type,
                    strategy = strategy.label(),
                    count = items.len(),
                    "access strategy succeeded"
                );
                return Ok(items);
            }
            Ok(_) => {
                reached_but_empty = true;
                tracing::debug!(
                    target: "4da::sources::access",
                    source = source_type,
                    strategy = strategy.label(),
                    "access strategy reached source but returned no items; trying next"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "4da::sources::access",
                    source = source_type,
                    strategy = strategy.label(),
                    error = %e,
                    "access strategy failed; trying next"
                );
                errors.push(e);
            }
        }
    }

    // At least one path reached the source and reported no data → that is the honest answer, even if
    // other paths errored (those alternates simply could not improve on "reachable but empty").
    if reached_but_empty {
        return Ok(Vec::new());
    }

    Err(errors
        .into_iter()
        .max_by_key(actionability)
        .unwrap_or_else(|| {
            SourceError::Other(format!("{source_type}: no access strategies configured"))
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Canned {
        label: &'static str,
        result: std::sync::Mutex<Option<SourceResult<Vec<SourceItem>>>>,
    }

    impl Canned {
        fn ok(label: &'static str, n: usize) -> Box<dyn AccessStrategy> {
            let items = (0..n)
                .map(|i| SourceItem::new("test", &format!("id{i}"), "title"))
                .collect();
            Self::boxed(label, Ok(items))
        }
        fn err(label: &'static str, e: SourceError) -> Box<dyn AccessStrategy> {
            Self::boxed(label, Err(e))
        }
        fn boxed(
            label: &'static str,
            result: SourceResult<Vec<SourceItem>>,
        ) -> Box<dyn AccessStrategy> {
            Box::new(Canned {
                label,
                result: std::sync::Mutex::new(Some(result)),
            })
        }
    }

    #[async_trait]
    impl AccessStrategy for Canned {
        fn label(&self) -> &str {
            self.label
        }
        async fn fetch(&self) -> SourceResult<Vec<SourceItem>> {
            self.result
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Ok(Vec::new()))
        }
    }

    #[tokio::test]
    async fn first_success_wins_and_short_circuits() {
        let strategies = vec![
            Canned::ok("primary", 3),
            Canned::err("fallback", SourceError::Network("x".into())),
        ];
        let out = resilient_fetch("test", &strategies).await.unwrap();
        assert_eq!(out.len(), 3);
    }

    #[tokio::test]
    async fn fails_over_from_error_to_success() {
        let strategies = vec![
            Canned::err("primary", SourceError::RateLimited("429".into())),
            Canned::ok("fallback", 2),
        ];
        let out = resilient_fetch("test", &strategies).await.unwrap();
        assert_eq!(
            out.len(),
            2,
            "should use the fallback after the primary errored"
        );
    }

    #[tokio::test]
    async fn empty_success_beats_errors() {
        // A reachable-but-empty path is the honest answer even if another path errored.
        let strategies = vec![
            Canned::err("primary", SourceError::Network("down".into())),
            Canned::ok("fallback", 0),
        ];
        let out = resilient_fetch("test", &strategies).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn all_fail_surfaces_most_actionable_error() {
        // Forbidden (needs a credential) must win over a transient network error, regardless of order.
        let strategies = vec![
            Canned::err("a", SourceError::Network("blip".into())),
            Canned::err("b", SourceError::Forbidden("needs key".into())),
            Canned::err("c", SourceError::Parse("bad body".into())),
        ];
        let err = resilient_fetch("test", &strategies).await.unwrap_err();
        assert!(matches!(err, SourceError::Forbidden(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn empty_strategy_list_is_other_error() {
        let err = resilient_fetch("test", &[]).await.unwrap_err();
        assert!(matches!(err, SourceError::Other(_)));
    }

    #[test]
    fn actionability_ranks_forbidden_highest() {
        assert!(
            actionability(&SourceError::Forbidden("".into()))
                > actionability(&SourceError::RateLimited("".into()))
        );
        assert!(
            actionability(&SourceError::RateLimited("".into()))
                > actionability(&SourceError::Network("".into()))
        );
        assert!(
            actionability(&SourceError::Network("".into()))
                > actionability(&SourceError::Other("".into()))
        );
    }
}

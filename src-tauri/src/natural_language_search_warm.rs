// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Keep the first search fast: a bounded query embed, a query-embedding cache,
//! and a pre-warm that pays the cold costs before anyone is waiting.
//!
//! The cold costs are the query embedder (Ollama unloads `nomic-embed-text`
//! after its keep-alive, 30 minutes on the operator's machine) and the first
//! touch of the search connection's pages. The pre-warm embeds through the
//! SAME `embed_texts` path a search uses, so the model is loaded with the same
//! options (`num_gpu: 0` on a local Ollama) and Ollama never reloads it for the
//! next request.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tracing::{debug, info};

use super::natural_language_search_engine::run_blocking;
use super::natural_language_search_timing::{EmbedSource, StageClock};

/// How long a search waits for its query embedding before answering from the
/// keyword legs alone. A resident embedder answers a one-line query in tens of
/// milliseconds; only a cold load or a starved CPU takes longer. The embed keeps
/// running after the budget and lands in the cache, so the retry the palette
/// makes (`semantic_pending`) gets the full hybrid ranking.
pub(crate) const SEARCH_EMBED_BUDGET: Duration = Duration::from_millis(1_200);

/// Distinct query texts kept. A palette session types a handful of prefixes.
const QUERY_EMBED_CACHE_MAX: usize = 128;

/// A pre-warm is skipped when one finished this recently: opening the palette
/// repeatedly must not queue repeated embeds.
const WARM_MIN_INTERVAL: Duration = Duration::from_secs(30);

/// Text the pre-warm embeds. Any short text loads the model.
const WARM_TEXT: &str = "search";

/// The result of asking for one query's embedding.
#[derive(Debug, Clone, Default)]
pub(crate) struct EmbeddedQuery {
    pub vector: Option<Vec<f32>>,
    pub load_ms: Option<u64>,
    pub source: EmbedSource,
}

/// Small LRU of query text -> raw (un-weighted) embedding.
#[derive(Default)]
pub(crate) struct QueryEmbedCache {
    entries: VecDeque<(String, Vec<f32>)>,
}

impl QueryEmbedCache {
    pub(crate) fn get(&mut self, key: &str) -> Option<Vec<f32>> {
        let pos = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(pos)?;
        let vector = entry.1.clone();
        self.entries.push_back(entry);
        Some(vector)
    }

    pub(crate) fn insert(&mut self, key: String, vector: Vec<f32>) {
        self.entries.retain(|(k, _)| *k != key);
        self.entries.push_back((key, vector));
        while self.entries.len() > QUERY_EMBED_CACHE_MAX {
            self.entries.pop_front();
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

static QUERY_EMBED_CACHE: Lazy<Mutex<QueryEmbedCache>> =
    Lazy::new(|| Mutex::new(QueryEmbedCache::default()));

/// Cache keys whose embed is running right now.
static IN_FLIGHT: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Clears an in-flight key however the embed task ends (even by panic), so a
/// failed embed never leaves later searches waiting on a key nobody will fill.
struct InFlightGuard(String);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        IN_FLIGHT.lock().remove(&self.0);
    }
}

/// The cache key carries the model and the embedding space, so a model switch or
/// a re-embed never serves a vector from the old space.
pub(crate) fn cache_key(model: &str, epoch: u64, text: &str) -> String {
    format!("{model}\u{1f}{epoch}\u{1f}{text}")
}

fn current_cache_key(text: &str) -> String {
    cache_key(
        &crate::reembed::get_embedding_model(),
        crate::reembed_space::embedding_epoch(),
        text,
    )
}

fn is_real_vector(v: &[f32]) -> bool {
    v.iter().any(|&x| x != 0.0)
}

/// Embed one text through the app's embed path, recording the model load time.
/// Zero vectors (the no-provider fallback) are not embeddings and are dropped.
async fn embed_one(text: String) -> (Option<Vec<f32>>, Option<u64>) {
    let texts = [text];
    let (result, load_ms) =
        crate::embeddings::observe_embed_load(crate::embeddings::embed_texts(&texts)).await;
    let vector = result
        .ok()
        .and_then(|mut v| v.pop())
        .filter(|v| is_real_vector(v));
    (vector, load_ms)
}

/// The query's raw embedding: from the cache, or embedded within
/// [`SEARCH_EMBED_BUDGET`]. Past the budget the search proceeds on its keyword
/// legs and the embed finishes in the background into the cache.
pub(crate) async fn embed_query_text(text: &str) -> EmbeddedQuery {
    embed_query_text_within(text, SEARCH_EMBED_BUDGET).await
}

pub(crate) async fn embed_query_text_within(text: &str, budget: Duration) -> EmbeddedQuery {
    let key = current_cache_key(text);
    if let Some(vector) = QUERY_EMBED_CACHE.lock().get(&key) {
        return EmbeddedQuery {
            vector: Some(vector),
            load_ms: None,
            source: EmbedSource::Cached,
        };
    }
    // The same text is already being embedded (a retry after `semantic_pending`,
    // or a pre-warm): wait for that one instead of queueing a duplicate.
    if !IN_FLIGHT.lock().insert(key.clone()) {
        return await_in_flight(&key, budget).await;
    }
    let owned = text.to_string();
    let task = tokio::spawn(async move {
        let _in_flight = InFlightGuard(key.clone());
        let (vector, load_ms) = embed_one(owned).await;
        if let Some(v) = &vector {
            QUERY_EMBED_CACHE.lock().insert(key, v.clone());
        }
        (vector, load_ms)
    });
    match tokio::time::timeout(budget, task).await {
        Ok(Ok((Some(vector), load_ms))) => EmbeddedQuery {
            vector: Some(vector),
            load_ms,
            source: EmbedSource::Fresh,
        },
        Ok(Ok((None, load_ms))) => EmbeddedQuery {
            vector: None,
            load_ms,
            source: EmbedSource::Unavailable,
        },
        Ok(Err(_)) => EmbeddedQuery {
            source: EmbedSource::Unavailable,
            ..EmbeddedQuery::default()
        },
        Err(_) => {
            info!(
                target: "4da::search",
                budget_ms = budget.as_millis() as u64,
                "Query embedder not ready within the search budget; answering from keyword legs, embed continues in background"
            );
            EmbeddedQuery {
                source: EmbedSource::TimedOut,
                ..EmbeddedQuery::default()
            }
        }
    }
}

/// Wait up to `budget` for an embed another caller started to reach the cache.
async fn await_in_flight(key: &str, budget: Duration) -> EmbeddedQuery {
    const POLL: Duration = Duration::from_millis(25);
    let deadline = Instant::now() + budget;
    loop {
        if let Some(vector) = QUERY_EMBED_CACHE.lock().get(key) {
            return EmbeddedQuery {
                vector: Some(vector),
                load_ms: None,
                source: EmbedSource::Cached,
            };
        }
        // The other embed finished without a vector (no provider): stop waiting.
        if !IN_FLIGHT.lock().contains(key) {
            return EmbeddedQuery {
                source: EmbedSource::Unavailable,
                ..EmbeddedQuery::default()
            };
        }
        if Instant::now() >= deadline {
            return EmbeddedQuery {
                source: EmbedSource::TimedOut,
                ..EmbeddedQuery::default()
            };
        }
        tokio::time::sleep(POLL).await;
    }
}

/// What one pre-warm did, in milliseconds per stage.
#[derive(Debug, Clone, Default)]
pub(crate) struct WarmReport {
    pub embed_ms: u64,
    pub embed_load_ms: Option<u64>,
    pub ace_ms: u64,
    pub db_ms: u64,
}

static WARMING: AtomicBool = AtomicBool::new(false);
static LAST_WARM: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));

/// Whether a pre-warm should start now: none in flight, none finished within
/// [`WARM_MIN_INTERVAL`]. Claims the in-flight slot when it returns true.
pub(crate) fn try_begin_warm(now: Instant, last: Option<Instant>, in_flight: &AtomicBool) -> bool {
    if last.is_some_and(|t| now.saturating_duration_since(t) < WARM_MIN_INTERVAL) {
        return false;
    }
    in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

/// Pay the search path's cold costs now: load the query embedder, build the ACE
/// weighting inputs, and touch the search connection. Returns `None` when skipped
/// (one is running or ran within [`WARM_MIN_INTERVAL`], or search is not entitled).
pub(crate) async fn prewarm_search(reason: &'static str) -> Option<WarmReport> {
    if crate::settings::require_signal_feature("natural_language_query").is_err() {
        return None;
    }
    if !try_begin_warm(Instant::now(), *LAST_WARM.lock(), &WARMING) {
        debug!(target: "4da::search", reason, "Search pre-warm skipped (recent or in flight)");
        return None;
    }
    let report = run_prewarm().await;
    *LAST_WARM.lock() = Some(Instant::now());
    WARMING.store(false, Ordering::Release);
    info!(
        target: "4da::search",
        reason,
        embed_ms = report.embed_ms,
        embed_load_ms = ?report.embed_load_ms,
        ace_ms = report.ace_ms,
        db_ms = report.db_ms,
        "Search pre-warmed"
    );
    Some(report)
}

async fn run_prewarm() -> WarmReport {
    let mut report = WarmReport::default();
    let mut clock = StageClock::start();
    let (warm_vector, load_ms) = embed_one(WARM_TEXT.to_string()).await;
    report.embed_load_ms = load_ms;
    report.embed_ms = clock.lap();

    let ace_ctx = run_blocking(|| Ok(crate::scoring::get_ace_context()))
        .await
        .unwrap_or_default();
    let _ = crate::scoring::get_topic_embeddings(&ace_ctx).await;
    report.ace_ms = clock.lap();

    // A real KNN, not just a connection touch: the vector scan reads every stored
    // vector (~510 MB on the 2 GB corpus), and a fresh process measured 7,085 ms
    // for its first one against ~480 ms once those pages were cached.
    let vector = warm_vector.unwrap_or_default();
    let _ = run_blocking(move || {
        let db = crate::get_database()
            .map_err(|e| crate::error::FourDaError::Internal(format!("DB: {e}")))?;
        let _ = db.hybrid_search_timed(WARM_TEXT, &vector, 1, 0.4, 0.6);
        Ok(())
    })
    .await;
    report.db_ms = clock.lap();
    report
}

/// Pre-warm the search path when the search palette opens, so the embedder
/// loads while the user is still typing. Fire-and-forget: returns at once.
#[tauri::command]
pub async fn warm_search() -> crate::error::Result<()> {
    tokio::spawn(prewarm_search("palette-open"));
    Ok(())
}

#[cfg(test)]
#[path = "natural_language_search_warm_tests.rs"]
mod tests;

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Per-stage timings for one search, logged on the "Search completed" line.
//!
//! A single `ms=` total could not say where a 14 s first search went
//! (measured 2026-10-10), so every stage that can stall is timed on its own.

use std::time::Instant;

use crate::db::hybrid_search::HybridSearchTimings;

/// Where the query vector came from.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmbedSource {
    /// Embedded for this search.
    #[default]
    Fresh,
    /// Reused from the query-embedding cache.
    Cached,
    /// The embedder did not answer within the search budget: keyword legs only.
    TimedOut,
    /// No embedding (no keywords, or no provider): keyword legs only.
    Unavailable,
}

impl EmbedSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Cached => "cached",
            Self::TimedOut => "timed_out",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Milliseconds per search stage. There is no LLM call and no gap detection on
/// this path (both removed; see `natural_language_query`), so neither has a field.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct SearchTimings {
    /// Language detection, keyword extraction, intent.
    pub parse_ms: u64,
    /// Stack context + related decisions.
    pub context_ms: u64,
    /// Query translation for the keyword legs (non-English queries only).
    pub translate_ms: u64,
    /// Waiting for the query embedding (zero on a cache hit).
    pub embed_ms: u64,
    /// Ollama's `load_duration` for that embed: well above zero means the model
    /// was not resident and this search paid for loading it.
    pub embed_load_ms: Option<u64>,
    pub embed_source: EmbedSource,
    /// ACE context + topic embeddings for the query weighting.
    pub ace_ms: u64,
    pub hybrid: HybridSearchTimings,
    /// Stack boost + final sort.
    pub rank_ms: u64,
}

/// Lap timer: each `lap` returns the milliseconds since the previous one.
pub(crate) struct StageClock(Instant);

impl StageClock {
    pub(crate) fn start() -> Self {
        Self(Instant::now())
    }

    pub(crate) fn lap(&mut self) -> u64 {
        let ms = self.0.elapsed().as_millis() as u64;
        self.0 = Instant::now();
        ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laps_measure_each_stage_separately() {
        let mut clock = StageClock::start();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let first = clock.lap();
        let second = clock.lap();
        assert!(first >= 15, "first lap measured the sleep: {first}");
        assert!(second < first, "second lap restarted the clock: {second}");
    }

    #[test]
    fn embed_sources_have_stable_log_names() {
        let names: Vec<_> = [
            EmbedSource::Fresh,
            EmbedSource::Cached,
            EmbedSource::TimedOut,
            EmbedSource::Unavailable,
        ]
        .iter()
        .map(|s| s.as_str())
        .collect();
        assert_eq!(names, ["fresh", "cached", "timed_out", "unavailable"]);
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Which sources may appear in the Signal feed, and for how long a news item
//! stays in the live list. Two tables, both read by source type only, so the
//! verdict persist boundary, the reconciliation sweeps and the display-set
//! pass (which hold no `Source`) cannot drift apart.
//!
//! Neither rule touches a score: the feed is a VIEW over the scored corpus,
//! and both rules only decide what that view admits (no `PIPELINE_VERSION`
//! bump, AD-034).

use super::freshness::news_max_item_age_days;

/// Sources that are fetched and scored but never enter the feed.
///
/// Measured 2026-10-04 on AI-labelled gold over the shown Signal list: reddit
/// 0 of 14 useful, lemmy 0 of 4, youtube 0 of 2 — 0 of 20 together (earlier
/// label runs: reddit 1 of 8 and 1 of 12). Every other source that held a
/// slot that day produced at least one useful item. The operator cut these
/// three from the FEED only (2026-10-04, signal-lanes program): their rows
/// are still fetched, embedded and scored, so the surfaces that read the
/// corpus rather than the feed verdict (trends / topic hotness, Blind Spots,
/// knowledge gaps, search) keep them.
///
/// Enforced as the durable verdict reason `source_not_in_feed`
/// (`db::verdicts`), applied at THE persist boundary and by the
/// reconciliation sweep, and the display set converges on it (AD-039).
/// Revisit by re-labelling a sample of these sources' top-scored rows; to
/// readmit one, remove it here — the sweep and boundary follow, and the
/// risen sweep never resurrects a reasoned rejection, so readmitted rows
/// re-enter only as new items are scored.
pub(crate) const FEED_EXCLUDED_SOURCES: &[&str] = &["reddit", "lemmy", "youtube"];

/// Whether items of `source_type` may enter the feed at all.
pub(crate) fn admits_to_feed(source_type: &str) -> bool {
    !FEED_EXCLUDED_SOURCES.contains(&source_type)
}

/// SQL list literal of [`FEED_EXCLUDED_SOURCES`], for queries.
pub(crate) fn feed_excluded_sources_sql() -> String {
    FEED_EXCLUDED_SOURCES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How long a news item stays in the live feed, in days, for every source
/// the news max-age table bounds except Lobste.rs.
///
/// Measured 2026-10-04 on the live instance: of 471 recorded interactions
/// (click, scroll, ignore, engagement) with news-source items, 443 happened
/// within 1.3 days of publication (median 6 hours) and NONE between day 7 and
/// day 30 — the remaining 28 were archive leaks hundreds of days old. 21 days
/// is three times the last observed engagement age: it keeps a weekly
/// newsletter's previous two issues and anything a reader returning after a
/// fortnight away has not seen, and removes "This Week in Rust 667/668" and
/// August release announcements that had been sitting in the list for 25-45
/// days.
pub(crate) const NEWS_DISPLAY_WINDOW_DAYS: u32 = 21;

/// Lobste.rs keeps its 14-day bound: it is already the fetch-time story gate
/// (`lobsters.rs`, #782) and the ingest max age, so a lobsters item never
/// outlives the window in which it could have been ingested.
pub(crate) const LOBSTERS_DISPLAY_WINDOW_DAYS: u32 = 14;

/// The live-feed window for `source_type`, or `None` when the source's items
/// never age out of the feed. Exactly the sources the news max-age table
/// bounds (`freshness::news_max_item_age_days`): registries, advisories,
/// research, GitHub and Stack Overflow are exempt for the reasons recorded
/// there. The window never exceeds the ingest bound.
pub(crate) fn news_display_window_days(source_type: &str) -> Option<u32> {
    let ingest_bound = news_max_item_age_days(source_type)?;
    let window = if source_type == "lobsters" {
        LOBSTERS_DISPLAY_WINDOW_DAYS
    } else {
        NEWS_DISPLAY_WINDOW_DAYS
    };
    Some(window.min(ingest_bound))
}

/// Whether a news row of `source_type`, `age_days` old by its effective date
/// (publication date, else first ingest), has left the live feed. Undated
/// rows (no age) and unbounded sources never age out; exactly the window is
/// still in.
pub(crate) fn aged_out_of_feed(source_type: &str, age_days: Option<f64>) -> bool {
    let Some(window) = news_display_window_days(source_type) else {
        return false;
    };
    age_days.is_some_and(|age| age > f64::from(window))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_measured_zero_yield_sources_are_cut() {
        for cut in ["reddit", "lemmy", "youtube"] {
            assert!(!admits_to_feed(cut), "{cut}");
        }
        for kept in [
            "hackernews",
            "devto",
            "mastodon",
            "lobsters",
            "rss",
            "bluesky",
            "crates_io",
            "npm_registry",
            "osv",
            "cve",
            "github",
            "arxiv",
        ] {
            assert!(admits_to_feed(kept), "{kept}");
        }
        assert_eq!(feed_excluded_sources_sql(), "'reddit', 'lemmy', 'youtube'");
    }

    #[test]
    fn display_window_covers_news_only() {
        assert_eq!(news_display_window_days("lobsters"), Some(14));
        for news in [
            "hackernews",
            "mastodon",
            "bluesky",
            "devto",
            "rss",
            "twitter",
            "producthunt",
        ] {
            assert_eq!(news_display_window_days(news), Some(21), "{news}");
        }
        for exempt in [
            "crates_io",
            "npm_registry",
            "pypi",
            "go_modules",
            "osv",
            "cve",
            "arxiv",
            "papers_with_code",
            "huggingface",
            "github",
            "stackoverflow",
            "unknown_source",
        ] {
            assert_eq!(news_display_window_days(exempt), None, "{exempt}");
        }
    }

    #[test]
    fn window_never_exceeds_the_ingest_bound() {
        for s in ["lobsters", "hackernews", "rss", "devto", "mastodon"] {
            let window = news_display_window_days(s).expect("news source");
            let ingest = news_max_item_age_days(s).expect("news source");
            assert!(window <= ingest, "{s}: {window} > {ingest}");
        }
    }

    #[test]
    fn age_out_boundary_is_strict_and_undated_rows_stay() {
        assert!(!aged_out_of_feed("rss", Some(21.0)));
        assert!(aged_out_of_feed("rss", Some(21.001)));
        assert!(!aged_out_of_feed("lobsters", Some(14.0)));
        assert!(aged_out_of_feed("lobsters", Some(14.5)));
        assert!(!aged_out_of_feed("hackernews", Some(0.2)));
        assert!(!aged_out_of_feed("rss", None));
        // A future date (clock skew) is not old.
        assert!(!aged_out_of_feed("devto", Some(-2.0)));
        // Registries and advisories never age out of the feed.
        assert!(!aged_out_of_feed("crates_io", Some(400.0)));
        assert!(!aged_out_of_feed("osv", Some(400.0)));
    }
}

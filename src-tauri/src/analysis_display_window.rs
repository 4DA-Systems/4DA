// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The live feed's freshness window: a news item is news for a while, then it
//! leaves the live list — not deleted, not re-scored, not re-judged.
//!
//! Measured 2026-10-04: the Signal list (269 shown rows) still carried
//! "This Week in Rust 667/668", "Announcing rustup 1.29.1" and "Announcing
//! Rust 1.98.0", 25 to 74 days old. Their verdicts are correct — each was
//! news when it arrived — and the `stale_news` rule (#789) deliberately judges
//! age AT INGEST so the durable verdict never flips as the clock moves. What
//! was missing is a window on the DISPLAY.
//!
//! ## Why a display-set rule and not a durable verdict
//!
//! Every database consumer of the curated corpus already windows by age (the
//! content graph's 7/14/30-day toggle, the Brief's "worth knowing" facts,
//! judge-agreement metrics); only the in-memory display set, which carries
//! curated rows forward cycle after cycle, had none. Writing an `aged_out`
//! verdict would rewrite curated history because time passed — the content
//! graph's 30-day view would lose its 21-to-30-day rows and first-curated
//! recall metrics would shrink — which is exactly what `stale_news` declined
//! to do. So the window binds where the gap is: the display set, on the one
//! cycle boundary every foreground / scheduled / headless path reaches
//! (`analysis_status::analyze_cached_content_inner`), right after the
//! durable-verdict convergence (AD-039).
//!
//! ## Properties
//!
//! - **Never flaps.** Age only grows, so a row that left never qualifies to
//!   return; nothing lifts an `aged:` exclusion (the verdict convergence
//!   touches only `verdict:` rows, the brief expiry only `brief:` rows).
//! - **Demote-only, scores untouched.** `relevant = false`, `excluded = true`,
//!   `excluded_by = "aged:<window>d"`; `relevance_score`, `rank_score` and the
//!   durable verdict are never written (no `PIPELINE_VERSION` bump, AD-034).
//! - **News only.** The window comes from
//!   `sources::feed_admission::news_display_window_days`, which covers exactly
//!   the sources the news max-age table bounds; registries, advisories,
//!   research, GitHub and Stack Overflow are exempt, and so is any row
//!   carrying backend-confirmed security truth (`is_critical_alert`, the same
//!   exemption AD-035 grants against narration verdicts).
//! - **Effective date** = `published_at`, else `created_at` (first ingest).
//!   Rows ingested before #789's timestamp wiring (live up to 2026-10-02:
//!   every HN / lemmy / mastodon / reddit row) have no publication date;
//!   first ingest is never earlier than publication, so the fallback can only
//!   UNDER-state age and never removes a row younger than the window.

use std::collections::HashMap;

use rusqlite::params;
use tracing::warn;

use crate::db::Database;
use crate::sources::feed_admission::{aged_out_of_feed, news_display_window_days};
use crate::types::SourceRelevance;

/// `excluded_by` prefix this pass writes. Owned by this pass alone.
const AGED_EXCLUSION_PREFIX: &str = "aged:";

/// A row the live list would show: relevant, and not excluded — except by a
/// Brief ordering verdict, which keeps `relevant = true` (AD-035/AD-039) and
/// so cannot shield a row from leaving the feed.
fn is_surfaced(r: &SourceRelevance) -> bool {
    r.relevant
        && (!r.excluded
            || r.excluded_by
                .as_deref()
                .is_some_and(|e| e.starts_with("brief:")))
}

/// Age in days of each row by its effective date (publication, else first
/// ingest). Rows missing from the table are absent.
fn effective_ages(db: &Database, ids: &[i64]) -> rusqlite::Result<HashMap<i64, f64>> {
    let conn = db.read_conn();
    let mut stmt = conn.prepare_cached(
        "SELECT julianday('now') - julianday(COALESCE(published_at, created_at))
         FROM source_items WHERE id = ?1",
    )?;
    let mut ages = HashMap::with_capacity(ids.len());
    for id in ids {
        let age: Option<f64> = match stmt.query_row(params![id], |r| r.get(0)) {
            Ok(age) => age,
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e),
        };
        if let Some(age) = age {
            ages.insert(*id, age);
        }
    }
    Ok(ages)
}

/// Take news rows older than their source's live-feed window out of the
/// display set. Returns how many rows left this pass. Fail-open: a failed
/// age probe leaves the display set as it was.
pub(crate) fn age_out_display_news(db: &Database, results: &mut [SourceRelevance]) -> usize {
    let candidates: Vec<i64> = results
        .iter()
        .filter(|r| {
            is_surfaced(r)
                && !r.is_critical_alert
                && news_display_window_days(&r.source_type).is_some()
        })
        .map(|r| r.id as i64)
        .collect();
    if candidates.is_empty() {
        return 0;
    }
    let ages = match effective_ages(db, &candidates) {
        Ok(ages) => ages,
        Err(e) => {
            warn!(
                target: "4da::verdicts",
                error = %e,
                "Feed age probe failed — display set left as the cycle scored it"
            );
            return 0;
        }
    };
    let mut aged = 0;
    for r in results.iter_mut() {
        let Some(&age) = ages.get(&(r.id as i64)) else {
            continue;
        };
        if !is_surfaced(r) || !aged_out_of_feed(&r.source_type, Some(age)) {
            continue;
        }
        let window = news_display_window_days(&r.source_type).unwrap_or_default();
        r.relevant = false;
        r.excluded = true;
        r.excluded_by = Some(format!("{AGED_EXCLUSION_PREFIX}{window}d"));
        aged += 1;
    }
    aged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    fn row(db: &Database, source: &str, key: &str, published: Option<&str>, created: &str) -> i64 {
        let id = insert_test_item(db, source, key, &format!("item {key}"), "");
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET published_at = ?1, created_at = ?2 WHERE id = ?3",
                params![published, created, id],
            )
            .expect("pin dates");
        id
    }

    fn days_ago(days: f64) -> String {
        let at = chrono::Utc::now() - chrono::Duration::seconds((days * 86_400.0) as i64);
        at.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn shown(id: i64, source: &str) -> SourceRelevance {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": format!("item {id}"),
            "url": null,
            "top_score": 0.8,
            "matches": [],
            "relevant": true,
            "source_type": source,
        }))
        .expect("minimal SourceRelevance deserializes")
    }

    #[test]
    fn news_past_the_window_leaves_and_everything_else_stays() {
        let db = test_db();
        let twir = row(&db, "rss", "a1", Some(&days_ago(25.2)), &days_ago(24.0));
        let fresh_rss = row(&db, "rss", "a2", Some(&days_ago(20.5)), &days_ago(20.4));
        // No publication date (pre-#789 HN row): first ingest decides.
        let old_hn = row(&db, "hackernews", "a3", None, &days_ago(30.0));
        let young_hn = row(&db, "hackernews", "a4", None, &days_ago(3.0));
        // Lobste.rs keeps its 14-day window.
        let lobsters = row(
            &db,
            "lobsters",
            "a5",
            Some(&days_ago(15.0)),
            &days_ago(14.9),
        );
        // Registries and advisories never age out of the feed.
        let release = row(
            &db,
            "crates_io",
            "a6",
            Some(&days_ago(190.0)),
            &days_ago(30.0),
        );
        let advisory = row(&db, "osv", "a7", Some(&days_ago(100.0)), &days_ago(90.0));
        // Backend-confirmed security truth is exempt even from a news source.
        let critical = row(&db, "hackernews", "a8", None, &days_ago(40.0));
        let mut results = vec![
            shown(twir, "rss"),
            shown(fresh_rss, "rss"),
            shown(old_hn, "hackernews"),
            shown(young_hn, "hackernews"),
            shown(lobsters, "lobsters"),
            shown(release, "crates_io"),
            shown(advisory, "osv"),
            shown(critical, "hackernews"),
        ];
        results[7].is_critical_alert = true;

        assert_eq!(age_out_display_news(&db, &mut results), 3);
        let left: Vec<(u64, Option<&str>)> = results
            .iter()
            .filter(|r| !r.relevant)
            .map(|r| (r.id, r.excluded_by.as_deref()))
            .collect();
        assert_eq!(
            left,
            vec![
                (twir as u64, Some("aged:21d")),
                (old_hn as u64, Some("aged:21d")),
                (lobsters as u64, Some("aged:14d")),
            ]
        );
        assert!(results.iter().filter(|r| r.relevant).all(|r| !r.excluded));
    }

    #[test]
    fn convergent_and_never_lifted() {
        let db = test_db();
        let id = row(&db, "devto", "b1", Some(&days_ago(22.0)), &days_ago(22.0));
        let mut results = vec![shown(id, "devto")];
        assert_eq!(age_out_display_news(&db, &mut results), 1);
        // A second pass finds nothing to do and keeps the exclusion.
        assert_eq!(age_out_display_news(&db, &mut results), 0);
        assert_eq!(results[0].excluded_by.as_deref(), Some("aged:21d"));
        // The durable-verdict convergence never lifts it, whatever the verdict.
        db.conn
            .lock()
            .execute(
                "UPDATE source_items SET feed_relevant = 1 WHERE id = ?1",
                params![id],
            )
            .expect("curate");
        let converged =
            crate::analysis_verdicts::converge_display_on_durable_verdicts(&db, &mut results);
        assert_eq!(converged.restored, 0);
        assert!(!results[0].relevant && results[0].excluded);
        // Nor the Brief's expiry pass.
        crate::brief_rejections::apply_brief_rejection_demotions(&mut results, &HashMap::new());
        assert_eq!(results[0].excluded_by.as_deref(), Some("aged:21d"));
        // And the durable verdict itself was never written.
        let durable: Option<i64> = db
            .conn
            .lock()
            .query_row(
                "SELECT feed_relevant FROM source_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(durable, Some(1));
    }

    #[test]
    fn a_brief_demotion_does_not_shield_an_aged_row_and_other_exclusions_stay() {
        let db = test_db();
        let briefed = row(&db, "mastodon", "c1", None, &days_ago(28.0));
        let user = row(&db, "mastodon", "c2", None, &days_ago(28.0));
        let mut results = vec![shown(briefed, "mastodon"), shown(user, "mastodon")];
        results[0].excluded = true;
        results[0].excluded_by = Some("brief:old news".to_string());
        results[1].relevant = false;
        results[1].excluded = true;
        results[1].excluded_by = Some("anti-topic:crypto".to_string());

        assert_eq!(age_out_display_news(&db, &mut results), 1);
        assert_eq!(results[0].excluded_by.as_deref(), Some("aged:21d"));
        assert!(!results[0].relevant);
        assert_eq!(results[1].excluded_by.as_deref(), Some("anti-topic:crypto"));
    }

    #[test]
    fn rows_missing_from_the_table_are_left_alone() {
        let db = test_db();
        let mut results = vec![shown(987_654, "rss")];
        assert_eq!(age_out_display_news(&db, &mut results), 0);
        assert!(results[0].relevant && !results[0].excluded);
    }
}

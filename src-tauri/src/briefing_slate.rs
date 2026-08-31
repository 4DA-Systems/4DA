// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Briefing slate selection (grounded-first).
//!
//! Split from `digest_commands.rs` for size hygiene (declared there via
//! `#[path]`). Selection doctrine lives with the code: a GROUNDED item
//! outranks any ungrounded one, a per-source cap stops one noisy source from
//! crowding the brief, and the DB fallback tops up from the cold-start floor
//! rather than serving a one-item brief.

use crate::error::{Result, ResultExt};

/// Final slate size: the LLM narration reads the top 20 and the deterministic
/// floor the top 10, so 30 leaves margin for both cuts.
pub(super) const BRIEF_SLATE_LIMIT: usize = 30;
/// Over-fetch factor for the DB fallback: pulling 3x the slate gives the
/// grounded-first partition room to promote grounded items that a raw
/// score-ordered LIMIT would have cut.
const BRIEF_CANDIDATE_FETCH: usize = 90;
/// Max items any single source_type may occupy in the slate, so one noisy
/// source can't crowd the brief.
pub(super) const BRIEF_MAX_PER_SOURCE: usize = 5;
/// DB-fallback relevance floor. 0.35 keeps score-0.1 noise out of the LLM slate
/// while staying below the 0.40 relevance threshold (recall margin).
const BRIEF_DB_SCORE_FLOOR: f64 = 0.35;
/// Cold-start floor: when the quality floor yields too little (thin context in
/// the first days, before scoring has personal signal), top up from the
/// historical permissive floor rather than serving an empty or one-item brief.
/// The 0.1 floor dates to v1.0.0 and is only load-bearing for this case.
const BRIEF_DB_COLDSTART_FLOOR: f64 = 0.1;
/// Minimum candidate count below which the cold-start top-up kicks in. Guards
/// the floor cliff: one item at 0.36 plus forty in [0.1, 0.35) must not yield
/// a one-item brief.
const BRIEF_MIN_CANDIDATES: usize = 5;

/// Fetch brief candidates from the DB at the quality floor, topping up from
/// the cold-start floor (deduped by id) when fewer than
/// `BRIEF_MIN_CANDIDATES` clear it. Above-floor items keep priority — the
/// grounded-first slate sort downstream re-orders by grounding and score.
pub(super) fn fetch_db_fallback_items(
    db: &crate::db::Database,
    period_start: chrono::DateTime<chrono::Utc>,
    user_lang: &str,
) -> Result<Vec<crate::db::DigestSourceItem>> {
    let mut items = db
        .get_relevant_items_since(
            period_start,
            BRIEF_DB_SCORE_FLOOR,
            BRIEF_CANDIDATE_FETCH,
            user_lang,
        )
        .context("Failed to fetch items")?;
    if items.len() >= BRIEF_MIN_CANDIDATES {
        return Ok(items);
    }
    let seen: std::collections::HashSet<i64> = items.iter().map(|i| i.id).collect();
    let top_up = db
        .get_relevant_items_since(
            period_start,
            BRIEF_DB_COLDSTART_FLOOR,
            BRIEF_CANDIDATE_FETCH,
            user_lang,
        )
        .context("Failed to fetch items")?;
    items.extend(top_up.into_iter().filter(|i| !seen.contains(&i.id)));
    Ok(items)
}

/// Order the Brief candidate slate grounded-first and apply a per-source cap.
///
/// Partition rule (intended product behavior): a GROUNDED item outranks any
/// UNGROUNDED item regardless of raw score — a grounded 0.6 ranks above an
/// ungrounded 0.9 — because score cannot separate signal from noise; a strong
/// dependency edge into the user's actual stack can. Ungrounded items are
/// deprioritized, never hard-dropped. Within each partition: score DESC.
///
/// The per-source cap only bites under competition: if the slate would
/// otherwise go unfilled, over-cap items backfill the remaining slots rather
/// than starving the brief.
pub(super) fn order_briefing_slate(
    mut items: Vec<crate::db::DigestSourceItem>,
    grounded_ids: &std::collections::HashSet<i64>,
    max_per_source: usize,
    limit: usize,
) -> Vec<crate::db::DigestSourceItem> {
    let rank = |a: &crate::db::DigestSourceItem, b: &crate::db::DigestSourceItem| {
        let grounded_a = grounded_ids.contains(&a.id);
        let grounded_b = grounded_ids.contains(&b.id);
        grounded_b.cmp(&grounded_a).then_with(|| {
            b.relevance_score
                .unwrap_or(0.0)
                .partial_cmp(&a.relevance_score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    };
    items.sort_by(rank);

    let mut per_source: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut slate: Vec<crate::db::DigestSourceItem> = Vec::with_capacity(limit);
    let mut over_cap: Vec<crate::db::DigestSourceItem> = Vec::new();
    for item in items {
        if slate.len() >= limit {
            break;
        }
        let seen = per_source.entry(item.source_type.clone()).or_insert(0);
        if *seen < max_per_source {
            *seen += 1;
            slate.push(item);
        } else {
            over_cap.push(item);
        }
    }
    for item in over_cap {
        if slate.len() >= limit {
            break;
        }
        slate.push(item);
    }
    // Backfilled items re-enter in grounded-first order so downstream top-N
    // cuts (top-10 deterministic, top-20 narrated) stay grounding-faithful.
    slate.sort_by(rank);
    slate
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Grounded-first slate ordering
    // ========================================================================

    fn slate_item(id: i64, source_type: &str, score: f64) -> crate::db::DigestSourceItem {
        crate::db::DigestSourceItem {
            id,
            title: format!("item-{id}"),
            url: None,
            source_type: source_type.to_string(),
            created_at: chrono::Utc::now(),
            relevance_score: Some(score),
            topics: vec![],
            content_type: None,
        }
    }

    fn grounded(ids: &[i64]) -> std::collections::HashSet<i64> {
        ids.iter().copied().collect()
    }

    #[test]
    fn grounded_low_score_outranks_ungrounded_high_score() {
        // Intended product behavior: the grounded-first partition means a grounded
        // 0.6 item DOES outrank an ungrounded 0.9 item — score cannot separate
        // signal from noise; a verified dependency edge can.
        let items = vec![
            slate_item(1, "hackernews", 0.9), // ungrounded
            slate_item(2, "rss", 0.6),        // grounded
        ];
        let slate = order_briefing_slate(items, &grounded(&[2]), 5, 30);
        assert_eq!(
            slate.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![2, 1],
            "grounded item must lead the slate despite the lower score"
        );
    }

    #[test]
    fn ungrounded_items_are_deprioritized_not_dropped() {
        let items = vec![
            slate_item(1, "hackernews", 0.9), // ungrounded
            slate_item(2, "rss", 0.6),        // grounded
            slate_item(3, "reddit", 0.8),     // ungrounded
            slate_item(4, "github", 0.5),     // grounded
        ];
        let slate = order_briefing_slate(items, &grounded(&[2, 4]), 5, 30);
        assert_eq!(
            slate.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![2, 4, 1, 3],
            "grounded by score DESC, then ungrounded by score DESC — nothing dropped"
        );
    }

    #[test]
    fn per_source_cap_limits_noisy_source_under_competition() {
        // 8 high-scoring items from one noisy source + 3 from another: the noisy
        // source is capped at 5 in the slate; the quieter source still gets in.
        let mut items: Vec<_> = (1..=8)
            .map(|i| slate_item(i, "hackernews", 0.9 - (i as f64) * 0.01))
            .collect();
        items.extend((9..=11).map(|i| slate_item(i, "rss", 0.5)));
        let slate = order_briefing_slate(items, &grounded(&[]), 5, 8);
        assert_eq!(slate.len(), 8);
        let hn = slate
            .iter()
            .filter(|i| i.source_type == "hackernews")
            .count();
        let rss = slate.iter().filter(|i| i.source_type == "rss").count();
        assert_eq!(hn, 5, "noisy source capped at 5");
        assert_eq!(rss, 3, "competing source fills the remaining slots");
    }

    #[test]
    fn per_source_cap_backfills_when_no_competition() {
        // All candidates come from one source: the cap must not starve the brief
        // — over-cap items backfill because there is nothing to crowd out.
        let items: Vec<_> = (1..=8)
            .map(|i| slate_item(i, "hackernews", 0.9 - (i as f64) * 0.01))
            .collect();
        let slate = order_briefing_slate(items, &grounded(&[]), 5, 10);
        assert_eq!(slate.len(), 8, "single-source slate backfills past the cap");
    }

    // ========================================================================
    // DB-fallback floor (0.35 quality floor, 0.1 cold-start top-up)
    // ========================================================================

    fn insert_scored_item(db: &crate::db::Database, title: &str, score: f64) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO source_items (source_type, source_id, url, title, content, content_hash, embedding, relevance_score, created_at)
             VALUES ('test', ?1, NULL, ?1, '', ?1, X'', ?2, datetime('now'))",
            rusqlite::params![title, score],
        )
        .expect("insert scored source_item");
    }

    #[test]
    fn db_fallback_floor_drops_low_score_noise_when_enough_better_items_exist() {
        let db = crate::test_utils::test_db();
        for i in 1..=5 {
            insert_scored_item(&db, &format!("good item {i}"), 0.5);
        }
        insert_scored_item(&db, "noise item", 0.15);

        let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
        let items = fetch_db_fallback_items(&db, period_start, "en").expect("fetch");
        assert_eq!(
            items.len(),
            5,
            "with >= BRIEF_MIN_CANDIDATES above the floor, 0.15 noise stays out"
        );
        assert!(items.iter().all(|i| i.title != "noise item"));
    }

    #[test]
    fn db_fallback_tops_up_at_coldstart_floor_when_nothing_clears_quality_floor() {
        let db = crate::test_utils::test_db();
        insert_scored_item(&db, "thin-context item", 0.2);

        let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
        let items = fetch_db_fallback_items(&db, period_start, "en").expect("fetch");
        assert_eq!(
            items.len(),
            1,
            "cold-start: with nothing >= 0.35 the 0.1 floor keeps the brief alive"
        );
        assert_eq!(items[0].title, "thin-context item");
    }

    #[test]
    fn db_fallback_tops_up_past_floor_cliff_without_duplicates() {
        // Floor cliff: one item at 0.36 + several in [0.1, 0.35) must not yield
        // a one-item brief. The top-up merges the sub-floor items and dedups by
        // id (the 0.36 item also matches the 0.1 fetch).
        let db = crate::test_utils::test_db();
        insert_scored_item(&db, "just above floor", 0.36);
        for i in 1..=3 {
            insert_scored_item(&db, &format!("sub-floor item {i}"), 0.2);
        }

        let period_start = chrono::Utc::now() - chrono::Duration::hours(72);
        let items = fetch_db_fallback_items(&db, period_start, "en").expect("fetch");
        assert_eq!(items.len(), 4, "top-up fills past the cliff, deduped by id");
        assert_eq!(
            items
                .iter()
                .filter(|i| i.title == "just above floor")
                .count(),
            1,
            "above-floor item must not be duplicated by the top-up fetch"
        );
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Corroboration context construction for the scoring pipeline.
//!
//! The signal-classification and quality-composite helpers that used to live
//! here were V1-only and were deleted with the V1 pipeline (2026-08-12); V2
//! carries its own `compute_quality_composite` and classifies signals inline.

use crate::db::Database;
use crate::signals;

use super::dependencies;

/// Build a real CorroborationContext from the database for a given item.
///
/// Queries:
/// 1. How many distinct source types have items about similar topics in the last 72 hours
/// 2. Whether any matched dependency confirms the signal
/// 3. Whether any open signal chain covers this topic and its current phase
pub(super) fn build_corroboration(
    db: &Database,
    topics: &[String],
    matched_deps: &[dependencies::DepMatch],
) -> signals::CorroborationContext {
    if topics.is_empty() {
        return signals::CorroborationContext::default();
    }

    // 1. Count distinct source types with items about the same topics in last 72 hours
    let source_count = {
        let conn = db.conn.lock();
        let topic_like_clauses: Vec<String> = topics
            .iter()
            .take(5) // Limit to top 5 topics for query performance
            .map(|t| {
                format!(
                    "LOWER(title) LIKE '%{}%'",
                    t.to_lowercase().replace('\'', "''")
                )
            })
            .collect();

        if topic_like_clauses.is_empty() {
            1_usize
        } else {
            let where_clause = topic_like_clauses.join(" OR ");
            let query = format!(
                "SELECT COUNT(DISTINCT source_type) FROM source_items \
                 WHERE created_at >= datetime('now', '-3 days') AND ({where_clause})"
            );
            conn.query_row(&query, [], |row| row.get::<_, i64>(0))
                .unwrap_or(1) as usize
        }
    };

    // 2. Dependency match — the single canonical grounding predicate. A bare
    //    non-dev hit is NOT enough; the classifier's Critical hard-gate trusts
    //    this flag, so it must mean the same "strongly grounded" as the
    //    evidence pool and the persisted link set (non-dev, confidence >= the
    //    strong floor, non-ambiguous name, and name-corroborated: the item
    //    actually names the package).
    let dependency_match = dependencies::is_strongly_grounded(matched_deps);

    // 3. Signal chain phase — detect if topics appear across multiple days
    //    (lightweight chain detection without the full detect_chains() machinery)
    let chain_phase = {
        let conn = db.conn.lock();
        let mut phase: Option<String> = None;
        for topic in topics.iter().take(3) {
            let topic_lower = topic.to_lowercase();
            // Count distinct days this topic has appeared in source items (last 7 days)
            let day_count: i64 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT DATE(created_at)) FROM source_items \
                     WHERE created_at >= datetime('now', '-7 days') AND LOWER(title) LIKE ?1",
                    rusqlite::params![format!("%{}%", topic_lower)],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if day_count >= 4 {
                phase = Some("peak".to_string());
                break;
            } else if day_count >= 3 {
                phase = Some("escalating".to_string());
                break;
            } else if day_count >= 2 && phase.is_none() {
                phase = Some("active".to_string());
            }
        }
        phase
    };

    signals::CorroborationContext {
        source_count,
        dependency_match,
        chain_phase,
    }
}

#[cfg(test)]
mod tests {
    use super::build_corroboration;
    use super::dependencies::{DepMatch, VersionDelta};
    use crate::db::Database;
    use crate::test_utils::{insert_test_item, test_db};

    /// Build a `DepMatch` for grounding tests. A non-dev, name-corroborated
    /// match with confidence >= `STRONG_GROUNDING_CONFIDENCE` (0.40) and a
    /// non-ambiguous name is "strongly grounded"; flip `is_dev`, drop the
    /// confidence, or clear `corroborated` to break it.
    fn dep(name: &str, confidence: f32, is_dev: bool) -> DepMatch {
        DepMatch {
            package_name: name.to_string(),
            confidence,
            version_delta: VersionDelta::Unknown,
            is_dev,
            is_direct: true,
            version: None,
            ecosystem: "rust".to_string(),
            corroborated: true,
            raw_name: None,
        }
    }

    /// Insert one item per entry in `day_offsets`, each titled with `topic`, and
    /// back-date its `created_at` by that many days. Drives the distinct-day
    /// chain-phase detection in `build_corroboration`.
    fn insert_topic_on_days(db: &Database, source_type: &str, topic: &str, day_offsets: &[i64]) {
        for (i, &days) in day_offsets.iter().enumerate() {
            let id = insert_test_item(
                db,
                source_type,
                &format!("{topic}-{i}"),
                &format!("{topic} update {i}"),
                "body",
            );
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET created_at = datetime('now', ?1) WHERE id = ?2",
                rusqlite::params![format!("-{days} days"), id],
            )
            .expect("backdate created_at");
        }
    }

    // ---- build_corroboration: real corroboration context from the DB ----

    #[test]
    fn corroboration_empty_topics_is_default() {
        // No topics → no DB work, the canonical default context. The default
        // is deliberately *restrictive*: source_count = 1 (single-source gate
        // applies), not 0, so an un-topiced item is never treated as corroborated.
        let db = test_db();
        let c = build_corroboration(&db, &[], &[]);
        assert_eq!(c.source_count, 1);
        assert!(!c.dependency_match);
        assert!(c.chain_phase.is_none());
    }

    #[test]
    fn corroboration_counts_distinct_source_types() {
        // Three different source types all talking about "rust" → source_count 3.
        let db = test_db();
        insert_test_item(&db, "hackernews", "a", "Rust 2.0 released", "body");
        insert_test_item(&db, "reddit", "b", "Why Rust wins", "body");
        insert_test_item(&db, "github", "c", "rust-lang/rust news", "body");
        // An unrelated item must not inflate the count.
        insert_test_item(&db, "lobsters", "d", "Python tips", "body");

        let c = build_corroboration(&db, &["rust".to_string()], &[]);
        assert_eq!(
            c.source_count, 3,
            "three distinct source types mention rust"
        );
        // All inserted same-day → only one distinct day → no chain phase.
        assert!(c.chain_phase.is_none());
    }

    #[test]
    fn corroboration_source_count_zero_when_no_title_match() {
        let db = test_db();
        insert_test_item(&db, "hackernews", "a", "Python tips", "body");
        let c = build_corroboration(&db, &["nonexistent-topic".to_string()], &[]);
        assert_eq!(c.source_count, 0);
    }

    #[test]
    fn corroboration_dependency_match_true_for_strong_grounding() {
        let db = test_db();
        // Non-dev, confident, non-ambiguous package name → strongly grounded.
        let c = build_corroboration(&db, &["x".to_string()], &[dep("tokio", 0.95, false)]);
        assert!(c.dependency_match);
    }

    #[test]
    fn corroboration_dependency_match_for_dev_and_weak_dep() {
        let db = test_db();
        // Dev dependency IS a grounding edge at strong confidence (item 16,
        // 2026-08-23: manifest devDeps ground the feed; only the Critical
        // paging lane stays non-dev via `is_strongly_grounded_direct`).
        let c1 = build_corroboration(&db, &["x".to_string()], &[dep("tokio", 0.95, true)]);
        assert!(c1.dependency_match);
        // Confidence below the 0.40 strong floor does not ground.
        let c2 = build_corroboration(&db, &["x".to_string()], &[dep("tokio", 0.30, false)]);
        assert!(!c2.dependency_match);
        // No deps at all.
        let c3 = build_corroboration(&db, &["x".to_string()], &[]);
        assert!(!c3.dependency_match);
    }

    #[test]
    fn corroboration_dependency_match_false_without_name_corroboration() {
        let db = test_db();
        // A confident non-dev match whose item never actually named the
        // package (subterm/topic-overlap hit) is NOT a grounding edge — the
        // 2026-07-02 phantom-critical class.
        let mut d = dep("tokio", 0.95, false);
        d.corroborated = false;
        let c = build_corroboration(&db, &["x".to_string()], &[d]);
        assert!(!c.dependency_match);
    }

    #[test]
    fn corroboration_chain_phase_active_escalating_peak() {
        // 2 distinct days → "active".
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "kubernetes", &[0, 1]);
        assert_eq!(
            build_corroboration(&db, &["kubernetes".to_string()], &[]).chain_phase,
            Some("active".to_string())
        );

        // 3 distinct days → "escalating".
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "kubernetes", &[0, 1, 2]);
        assert_eq!(
            build_corroboration(&db, &["kubernetes".to_string()], &[]).chain_phase,
            Some("escalating".to_string())
        );

        // 4+ distinct days → "peak".
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "kubernetes", &[0, 1, 2, 3]);
        assert_eq!(
            build_corroboration(&db, &["kubernetes".to_string()], &[]).chain_phase,
            Some("peak".to_string())
        );
    }

    #[test]
    fn corroboration_chain_phase_ignores_items_outside_7_day_window() {
        // Two appearances, but one is 10 days old → only one in-window day → no chain.
        let db = test_db();
        insert_topic_on_days(&db, "hackernews", "graphql", &[0, 10]);
        assert!(build_corroboration(&db, &["graphql".to_string()], &[])
            .chain_phase
            .is_none());
    }
}

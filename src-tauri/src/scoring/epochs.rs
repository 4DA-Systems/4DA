// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Scoped pipeline epochs — Phase 2 of scoring-drain elimination.
//!
//! A `PIPELINE_VERSION` bump marks the ENTIRE corpus stale, forcing a full
//! re-score (the drain) even when the scoring change can only affect a narrow
//! slice (e.g. a crates.io grounding fix cannot change how a Hacker News
//! discussion scores). This module lets a version bump declare the slice it
//! can affect, so everything else is *promoted* — re-stamped at the new
//! version untouched, its score provably unchanged — and only the affected
//! slice actually re-scores. On a 192k corpus that turns a multi-hour
//! whole-corpus drain (which also runs on every end-user's machine after an
//! app update) into a minutes-long slice drain.
//!
//! ## Contract
//!
//! When bumping `PIPELINE_VERSION`, you MAY register the new version in
//! [`SCOPED_EPOCHS`] with an **invalidation predicate**: a SQL boolean
//! expression over `source_items` columns that is TRUE for every item the
//! scoring change could possibly affect. Example registration for a
//! registry-release-only change:
//!
//! ```text
//! (18, "source_type IN ('crates_io','npm','pypi') \
//!       OR content_type IN ('release_notes','platform_update')")
//! ```
//!
//! Rules:
//! - **When unsure, do not register.** An unregistered bump gets NO
//!   promotion — the whole corpus drains, exactly the pre-epochs behavior.
//!   Over-broad predicates are safe (they only shrink the promotion);
//!   under-broad predicates are the one real hazard (an affected item would
//!   keep its stale score until organic re-scoring), so the predicate must
//!   be *provably* a superset of the change's reach.
//! - Predicates are compile-time constants (never user input) over indexed
//!   columns (`source_type`, `content_type` are both indexed) — no injection
//!   surface, no table scans beyond the stale set.
//! - Write predicates in **positive form** (equality / `IN` lists naming the
//!   affected values). A NULL column evaluates as *not matching* — i.e. the
//!   item is promoted. Negative-form predicates (`col != 'x'`) would make
//!   NULL rows promote too, which may not be what you mean — avoid them.
//! - Changes to global machinery (calibration sigmoid, gate tables, DSL
//!   weights, freshness tiers) affect everything — do NOT register those.
//!
//! ## Mechanics
//!
//! Staleness is implicit (`scored_pipeline_version < PIPELINE_VERSION`), so
//! promotion walks the version transitions in ascending order: for each
//! registered step `v`, items stamped `v-1` that do NOT match `v`'s
//! predicate are re-stamped to `v`. An item can ride consecutive promotions
//! (v16 → v17 → v18) but stops at the first step that is unregistered or
//! whose predicate matches it — from there the drain re-scores it straight
//! to the current version (a full PASIFA re-score subsumes every
//! intermediate change, so skipping the intermediate stamps is correct).
//! Never-scored items (`scored_pipeline_version = 0`) are structurally
//! untouched: the walk starts at the minimum *stamped* version (>= 1), so
//! `v-1 >= 1` for every step.
//!
//! Promotion is idempotent and cheap to probe: the fast path is a `MIN()`
//! over the `idx_source_items_scored_version` index (~0ms), so callers
//! invoke it unconditionally before drain work. Failure is fail-open to the
//! SAFE side: any SQL error aborts promotion and the full drain proceeds —
//! slower, never wrong.
//!
//! ## Bump per blast radius, not per release (AD-034)
//!
//! A `PIPELINE_VERSION` bump must correspond to a change in what `score_item`
//! writes to `relevance_score`. The batch-relative layer — cross-encoder
//! rerank, dedup corroboration boosts, domain/source diversity, per-source
//! percentile, the LLM advisor delta, the final rank cap — writes `top_score`
//! and `rank_score` only, and provably cannot move a stored evidence score.
//! Changing it must NOT bump the version.
//!
//! v26 is the cautionary case: five changes under one bump, one of them
//! (`apply_source_share_diversity`) a pure batch-layer cap that could not have
//! altered a single stored score. Bundling forced the union of five blast radii
//! onto the whole corpus AND made the bump unregisterable here, because no row
//! predicate can bound the reach of the widest member. Land scoped changes under
//! their own bumps and register each one.
//!
//! ## Why there is no per-AXIS registry (AD-033)
//!
//! The obvious generalisation of this module is to scope by signal axis rather
//! than by row — "this bump touches the dependency axis, so reuse everything
//! else" — since the row-predicate form cannot express a change to global gate
//! machinery, which is why v22, v25, v26 and v27 are all unregistered.
//!
//! It was measured instead of assumed, and the answer is that the one axis worth
//! materialising is already done. The context (KNN) axis was **95.8% of the cost
//! of scoring an item**, and it now lives in `item_context_cache`, keyed on the
//! CONTEXT-corpus generation and completely independent of `PIPELINE_VERSION` —
//! so a scoring bump already does not invalidate it. Every remaining axis
//! together is 2.7 ms/item; materialising them would save roughly eighteen
//! seconds on a whole-corpus drain. A registry nothing consumes is dead code,
//! which doctrine forbids. Revisit if some future axis acquires an input as
//! expensive as a vector scan.

use tracing::{info, warn};

use crate::db::Database;

/// Version → invalidation predicate registry. See the module docs for the
/// registration contract. v17 predates the mechanism and is fully drained, so
/// it is deliberately absent (an unregistered step = full drain = old, safe
/// behavior).
///
/// **v18 — look-alike registry-release categorical gate.** The only verdict
/// v18 changes is `relevant` for an *ungrounded registry release*, and
/// `pipeline_v2` computes that flag as
/// `dep_linker::is_registry_source(source_type) && matches!(content_type,
/// ReleaseNotes | BreakingChange) && !grounding.strong`. `is_registry_source`
/// is therefore a NECESSARY condition, which makes the source_type list below
/// a provable SUPERSET of the change's reach — it is that function's match arm
/// transcribed exactly (keep the two in sync if a registry is ever added).
///
/// Deliberately NOT narrowed further with `content_type IN (...)`: the stored
/// `content_type` column is a persisted classification while the flag uses the
/// value computed at score time, so intersecting on it could under-cover — the
/// one hazard the module contract names. Source type is assigned at ingest and
/// never re-derived, so it is safe to key on.
///
/// **v22 — deliberately absent** (2026-08-24 audit fix queue): the bump
/// changes global gate machinery (confirmation-gate evidence, keyword
/// confirmation, community signal, staleness evidence) — the module
/// contract's explicit do-not-register class. No predicate can provably
/// bound its reach, so the whole corpus drains. The mechanism stays for
/// future narrow bumps.
///
/// **v23 — superseded-release staleness floor.** The change deepens the
/// `published_at` staleness discount for ReleaseNotes and withholds the
/// grounded softening from them. `stale_published_multiplier` returns 1.0 for
/// anything at or below `fresh_months` (12), so an item whose published age is
/// under 12 months is arithmetically untouchable by this change — the
/// predicate below (published_at present AND older than 12 months) is a
/// provable SUPERSET of its reach. Deliberately NOT narrowed with
/// `content_type IN ('release_notes')`: the stored column is a persisted
/// classification while the multiplier uses the value computed at score time,
/// so intersecting on it could UNDER-cover — the one hazard the module
/// contract names. `published_at` is assigned at ingest and never re-derived,
/// so it is safe to key on (same reasoning as v18's source_type). NULL
/// published_at evaluates as not-matching → promoted, which is correct: those
/// items age from first-seen and carry no publication date to discount.
const SCOPED_EPOCHS: &[(i32, &str)] = &[
    (
        18,
        "source_type IN ('npm_registry','npm','crates_io','crates','pypi',\
     'go_modules','go','maven','nuget','packagist','rubygems','cocoapods')",
    ),
    (
        23,
        "published_at IS NOT NULL AND published_at < datetime('now','-12 months')",
    ),
    // v24 — superseded-release ceiling. The flag requires an age at or beyond
    // `superseded_months` (24), so nothing published inside 24 months can be
    // affected: the predicate is a provable superset of the change's reach.
    // Same reasoning as v23, one band deeper. `published_at` is assigned at
    // ingest and never re-derived, and NULL evaluates as not-matching →
    // promoted, which is correct (those items carry no publication date, so
    // the ceiling can never fire for them).
    (
        24,
        "published_at IS NOT NULL AND published_at < datetime('now','-24 months')",
    ),
];

/// Promote stale items that the registered epoch predicates prove unaffected,
/// re-stamping them at the version whose change cannot touch them. Returns
/// the number of promoted rows (0 when the corpus is current, no version is
/// registered, or the registry is empty). Errors are returned for the caller
/// to log — the drain then simply re-scores everything, which is always
/// correct.
pub(crate) fn promote_unaffected_stale(db: &Database) -> rusqlite::Result<u64> {
    promote_with_registry(db, super::PIPELINE_VERSION, SCOPED_EPOCHS)
}

/// Rows promoted per writer-lock hold. Measured on a copy of the live 2.2 GB
/// corpus (2026-07-25): one unchunked UPDATE re-stamping 141k rows held the
/// writer lock for ~86 s — pooled reads survive (WAL), but user WRITES
/// (save/dismiss/settings) would stall for the duration. Chunked AND with the
/// `idx_source_items_scored_version` index (Phase 100) present, the same
/// promotion measured 9.9 s total with a worst single hold of 1.4 s, and the
/// lock is released between chunks so interactive writes interleave. (Without
/// the index each chunk's subselect re-scans the table — the index is
/// load-bearing for this path, not just for the drain probes.)
const PROMOTION_CHUNK: usize = 10_000;

/// Core promotion walk, registry-injectable for tests.
fn promote_with_registry(
    db: &Database,
    current: i32,
    registry: &[(i32, &str)],
) -> rusqlite::Result<u64> {
    if registry.is_empty() {
        return Ok(0);
    }

    // Fast path: the minimum STAMPED version (>= 1 excludes the never-scored
    // backlog, which epochs must never touch). Uses the scored_pipeline_version
    // index — ~0ms. NULL means nothing is stamped yet.
    let min_stamped: Option<i64> = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT MIN(scored_pipeline_version) FROM source_items WHERE scored_pipeline_version >= 1",
            [],
            |r| r.get(0),
        )?
    };
    let Some(min_stamped) = min_stamped else {
        return Ok(0);
    };
    if min_stamped >= i64::from(current) {
        return Ok(0);
    }

    // Walk transitions ascending. Promotion is idempotent and each item's
    // stamp is independently correct, so per-chunk commits are safe: a crash
    // mid-walk leaves some items promoted and the rest stale — the stale ones
    // simply drain (correct, just slower), and the next promotion call
    // resumes where this one stopped.
    let mut promoted: u64 = 0;
    for step in (min_stamped + 1)..=i64::from(current) {
        let Some((_, predicate)) = registry.iter().find(|(v, _)| i64::from(*v) == step) else {
            // Unregistered step: no promotion — items below it stay stale and
            // take the full drain. This is the backward-compatible default.
            continue;
        };
        // `step - 1 >= 1` by construction (min_stamped >= 1), so the
        // never-scored backlog (version 0) is unreachable here.
        //
        // COALESCE(pred, 0): SQL three-valued logic — a NULL column (e.g. the
        // many legacy rows with NULL content_type) makes the raw predicate
        // NULL, and `NOT NULL` is NULL, which would silently exclude the row
        // from promotion and force a pointless re-score. For positive-form
        // predicates (IN lists / equality — the registration contract) a NULL
        // column genuinely does not match, i.e. the item is unaffected, so
        // NULL folds to "promote".
        let sql = format!(
            "UPDATE source_items SET scored_pipeline_version = {step} \
             WHERE rowid IN (SELECT rowid FROM source_items \
                             WHERE scored_pipeline_version = {prev} \
                               AND NOT COALESCE(({predicate}), 0) \
                             LIMIT {PROMOTION_CHUNK})",
            prev = step - 1,
        );
        loop {
            // Acquire → update one bounded chunk → release, so interactive
            // writers are never starved for more than one chunk's duration.
            let n = {
                let conn = db.conn.lock();
                conn.execute(&sql, [])?
            };
            promoted += n as u64;
            if n < PROMOTION_CHUNK {
                break;
            }
        }
    }

    if promoted > 0 {
        info!(
            target: "4da::epochs",
            promoted,
            current_version = current,
            "Scoped epoch promotion: unaffected items re-stamped without re-scoring"
        );
    }
    Ok(promoted)
}

/// Log-and-continue wrapper for drain call sites: promotion failure must
/// never block the drain (full re-score is the safe fallback).
pub(crate) fn promote_unaffected_stale_logged(db: &Database) -> u64 {
    match promote_unaffected_stale(db) {
        Ok(n) => n,
        Err(e) => {
            warn!(
                target: "4da::epochs",
                error = %e,
                "Epoch promotion failed — falling back to full drain (safe, slower)"
            );
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    fn stamp(db: &Database, id: i64, version: i32, score: f64) {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE source_items SET scored_pipeline_version = ?1, relevance_score = ?2 WHERE id = ?3",
            rusqlite::params![version, score, id],
        )
        .unwrap();
    }

    fn version_and_score(db: &Database, id: i64) -> (i32, Option<f64>) {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT scored_pipeline_version, relevance_score FROM source_items WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    /// A registered predicate promotes only NON-matching items; matching items
    /// stay stale for the drain, and promoted scores are byte-identical.
    #[test]
    fn promotes_unaffected_keeps_affected_stale() {
        let db = test_db();
        let hn = insert_test_item(&db, "hackernews", "hn1", "discussion", "x");
        let crates = insert_test_item(&db, "crates_io", "cr1", "release", "x");
        stamp(&db, hn, 17, 0.62);
        stamp(&db, crates, 17, 0.88);

        let registry = [(18, "source_type = 'crates_io'")];
        let promoted = promote_with_registry(&db, 18, &registry).unwrap();

        assert_eq!(promoted, 1, "exactly the non-matching item is promoted");
        assert_eq!(
            version_and_score(&db, hn),
            (18, Some(0.62)),
            "unaffected item re-stamped, score untouched"
        );
        assert_eq!(
            version_and_score(&db, crates),
            (17, Some(0.88)),
            "affected item stays stale for the drain"
        );
    }

    /// An unregistered bump promotes nothing — the whole corpus stays stale
    /// and takes the full drain. This IS the pre-epochs behavior; the peer's
    /// next version bump is untouched unless they opt in.
    #[test]
    fn unregistered_version_promotes_nothing() {
        let db = test_db();
        let a = insert_test_item(&db, "hackernews", "a", "t", "x");
        stamp(&db, a, 17, 0.5);

        // Registry knows about some OTHER version, not 18.
        let registry = [(12, "source_type = 'crates_io'")];
        assert_eq!(promote_with_registry(&db, 18, &registry).unwrap(), 0);
        assert_eq!(version_and_score(&db, a).0, 17, "item remains stale");

        // Truly empty registry short-circuits too.
        assert_eq!(promote_with_registry(&db, 18, &[]).unwrap(), 0);
    }

    /// Multi-step compounding: an item unaffected by BOTH registered steps
    /// rides v16 → v17 → v18; an item affected by the second step stops at
    /// the stamp before it.
    #[test]
    fn multi_step_promotion_compounds() {
        let db = test_db();
        let hn = insert_test_item(&db, "hackernews", "hn1", "t", "x");
        let crates = insert_test_item(&db, "crates_io", "cr1", "t", "x");
        stamp(&db, hn, 16, 0.4);
        stamp(&db, crates, 16, 0.7);

        let registry = [
            (17, "content_type = 'security_advisory'"),
            (18, "source_type = 'crates_io'"),
        ];
        let promoted = promote_with_registry(&db, 18, &registry).unwrap();

        // hn: matches neither predicate -> 16 -> 17 -> 18 (two promotions).
        assert_eq!(version_and_score(&db, hn).0, 18);
        // crates: clears step 17, is caught by step 18's predicate -> stops at 17.
        assert_eq!(version_and_score(&db, crates).0, 17);
        assert_eq!(promoted, 3, "hn twice + crates once");
    }

    /// A gap in registration blocks promotion past it: with step 17
    /// UNregistered, a v16 item cannot skip to 18 even if it fails 18's
    /// predicate — the unregistered 17 change might have affected it.
    #[test]
    fn unregistered_gap_blocks_promotion_past_it() {
        let db = test_db();
        let hn = insert_test_item(&db, "hackernews", "hn1", "t", "x");
        stamp(&db, hn, 16, 0.4);

        let registry = [(18, "source_type = 'crates_io'")];
        assert_eq!(promote_with_registry(&db, 18, &registry).unwrap(), 0);
        assert_eq!(
            version_and_score(&db, hn).0,
            16,
            "v16 item stays stale: the unregistered v17 step gates it"
        );
    }

    /// The never-scored backlog (version 0) is structurally untouchable, and
    /// an already-current corpus is a fast no-op.
    #[test]
    fn never_scored_untouched_and_current_corpus_noops() {
        let db = test_db();
        let unscored = insert_test_item(&db, "hackernews", "u1", "t", "x");
        // insert_test_item leaves version at 0 / score NULL.
        let current = insert_test_item(&db, "crates_io", "c1", "t", "x");
        stamp(&db, current, 18, 0.9);

        let registry = [(18, "source_type = 'nothing_matches_this'")];
        assert_eq!(promote_with_registry(&db, 18, &registry).unwrap(), 0);
        assert_eq!(
            version_and_score(&db, unscored).0,
            0,
            "never-scored items must never be stamped by promotion"
        );
    }

    /// SQL three-valued logic guard: a NULL column must read as "does not
    /// match" → the item is UNAFFECTED and gets promoted. Without the
    /// COALESCE fold, `NOT (NULL = 'x')` is NULL and the row would silently
    /// skip promotion, forcing a pointless full re-score of every legacy row
    /// whose content_type was never backfilled.
    #[test]
    fn null_column_counts_as_unaffected() {
        let db = test_db();
        let legacy = insert_test_item(&db, "hackernews", "leg1", "t", "x");
        stamp(&db, legacy, 17, 0.5);
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE source_items SET content_type = NULL WHERE id = ?1",
                rusqlite::params![legacy],
            )
            .unwrap();
        }

        let registry = [(18, "content_type IN ('release_notes','platform_update')")];
        assert_eq!(promote_with_registry(&db, 18, &registry).unwrap(), 1);
        assert_eq!(
            version_and_score(&db, legacy).0,
            18,
            "NULL content_type does not match the predicate → promoted"
        );
    }

    /// Every predicate in the REAL registry must be valid SQL over
    /// source_items — a typo fails here at test time, not at the first bump
    /// in production. (Trivially green while the registry is empty; the
    /// harness exists for the first registration.)
    #[test]
    fn real_registry_predicates_are_valid_sql() {
        let db = test_db();
        let conn = db.conn.lock();
        for (version, predicate) in SCOPED_EPOCHS {
            let sql = format!("SELECT COUNT(*) FROM source_items WHERE {predicate}");
            let result: Result<i64, _> = conn.query_row(&sql, [], |r| r.get(0));
            assert!(
                result.is_ok(),
                "registered predicate for v{version} is not valid SQL: {predicate}"
            );
        }
    }

    /// The logged wrapper swallows nothing on success and returns 0 on the
    /// empty registry (the production configuration today).
    #[test]
    fn logged_wrapper_is_noop_on_empty_registry() {
        let db = test_db();
        let a = insert_test_item(&db, "hackernews", "a", "t", "x");
        stamp(&db, a, 16, 0.5);
        assert_eq!(promote_unaffected_stale_logged(&db), 0);
        assert_eq!(version_and_score(&db, a).0, 16);
    }
}

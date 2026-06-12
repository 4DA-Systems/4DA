// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Review Sprint — explicit labeling of REAL corpus items.
//!
//! The taste test (15 curated cards) infers a persona but its synthetic
//! feedback is topic-level with no `source_item_id`, so the calibration
//! fitter never benefits from it. This module is the second phase: it
//! samples real `source_items` — preferring items that already have
//! unprocessed `calibration_samples` rows, topping up from scored items
//! when those run dry (no-LLM installs never stamp samples; their
//! labels still feed precision tracking and pre-pair future judgments)
//! — and asks the user for an explicit
//! relevant / not-relevant judgment. Each judgment becomes a
//! `feedback(source_item_id, relevant)` row — the strongest possible
//! label for `calibration_fitter::resolve_outcome` (explicit feedback
//! is unconditional, non-expiring ground truth). ~50 such labels and
//! the first confidence curve fits the same day instead of starving
//! for weeks on organic clicks alone.
//!
//! ## Sampling design
//!
//! A useful curve needs labels SPREAD across the score range — 50
//! labels all in the 0.7-1.0 band fit only one bucket. So sampling is
//! stratified two ways:
//!
//! - **Score bands** `[0,0.2) [0.2,0.45) [0.45,0.7) [0.7,1.0]` filled
//!   round-robin so each band contributes roughly equally.
//! - **Source diversity** — at most [`PER_SOURCE_CAP`] cards per
//!   `source_type`, so one firehose source can't dominate the sprint.
//!
//! Within a band, more recent samples win (recency = the score is
//! still comparable to what the current pipeline would produce).
//!
//! This is a LABELING task, not an intelligence surface: cards are
//! plain structs (mirroring the onboarding `TasteCard` precedent), not
//! `EvidenceItem`s — doctrine rule 4 governs ranked intelligence, not
//! ground-truth collection.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Result, ResultExt};

// ============================================================================
// Constants
// ============================================================================

/// Maximum cards returned by one sprint. Small enough to finish in a
/// few minutes, large enough that two sprints reach the fitter's
/// [`crate::calibration_fitter::MIN_FIT_SAMPLES`] floor.
pub const SPRINT_TARGET: usize = 24;

/// Maximum cards per `source_type` in one sprint.
pub const PER_SOURCE_CAP: usize = 4;

/// Candidate pool size pulled from SQL before stratification. Large
/// enough that every band/source bucket has options, small enough to
/// stay cheap.
const CANDIDATE_POOL: usize = 400;

/// Snippet length shown on a card (characters, post HTML-strip).
const SNIPPET_CHARS: usize = 200;

/// Relevance-score band edges. The last edge is exclusive-high 1.01 so
/// a score of exactly 1.0 lands in the top band.
const BAND_EDGES: [f32; 5] = [0.0, 0.2, 0.45, 0.7, 1.01];

/// Number of stratification bands.
const BAND_COUNT: usize = BAND_EDGES.len() - 1;

// ============================================================================
// Types
// ============================================================================

/// One labeling card in the review sprint. Plain card struct, NOT an
/// EvidenceItem — see module docs.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CalibrationSprintCard {
    /// Serialized as a plain JSON number (i64 ids fit comfortably).
    #[ts(type = "number")]
    pub source_item_id: i64,
    pub title: String,
    pub snippet: String,
    pub source_type: String,
    pub url: Option<String>,
}

/// Honest progress toward the first calibration fit. Every number here
/// is real and actionable: `labeled_total` is distinct items with
/// explicit feedback, `min_fit_samples` is the fitter's actual floor,
/// `curve_fitted` reports whether a curve file exists on disk.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
#[serde(rename_all = "camelCase")]
pub struct CalibrationSprintStatus {
    /// Distinct source items the user has explicitly labeled. Distinct
    /// (not raw row count) because one item's label resolves ALL of
    /// that item's calibration samples — repeat labels on the same
    /// item add no new fitter signal.
    #[ts(type = "number")]
    pub labeled_total: i64,
    /// Read from [`crate::calibration_fitter::MIN_FIT_SAMPLES`], never
    /// hardcoded here.
    pub min_fit_samples: u32,
    /// Whether any calibration curve has been fit and persisted.
    pub curve_fitted: bool,
}

/// Parsed sprint response. `Skip` deliberately writes nothing — an
/// unsure user must not inject noise into the ground truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SprintResponse {
    Relevant,
    NotRelevant,
    Skip,
}

/// Parse the frontend's response string. Unknown values are an error
/// at the command layer (None here).
pub fn parse_response(s: &str) -> Option<SprintResponse> {
    match s {
        "relevant" => Some(SprintResponse::Relevant),
        "not_relevant" => Some(SprintResponse::NotRelevant),
        "skip" => Some(SprintResponse::Skip),
        _ => None,
    }
}

/// A stratification candidate: the card plus the score it is bucketed
/// by. Candidates arrive ordered most-recent-first and stratify()
/// preserves that order within each band.
#[derive(Debug, Clone)]
pub struct SprintCandidate {
    pub card: CalibrationSprintCard,
    pub score: f32,
}

// ============================================================================
// Sampling
// ============================================================================

/// Band index for a relevance score. Scores outside [0, 1] clamp into
/// the edge bands (defensive — the pipeline already clamps).
fn band_index(score: f32) -> usize {
    for i in 0..BAND_COUNT {
        if score < BAND_EDGES[i + 1] {
            return i;
        }
    }
    BAND_COUNT - 1
}

/// Load the candidate pool: items that HAVE at least one unprocessed
/// calibration sample, have never been explicitly labeled, and have a
/// non-empty title. Ordered by the recency of their newest unprocessed
/// sample so stratification prefers fresh judgments.
fn load_candidates(conn: &Connection) -> Result<Vec<SprintCandidate>> {
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.content, si.source_type, si.url,
                    COALESCE(si.relevance_score, 0.0)
             FROM source_items si
             WHERE EXISTS (
                     SELECT 1 FROM calibration_samples cs
                     WHERE cs.source_item_id = si.id AND cs.processed_at IS NULL)
               AND NOT EXISTS (
                     SELECT 1 FROM feedback f WHERE f.source_item_id = si.id)
               AND TRIM(si.title) <> ''
             ORDER BY (SELECT MAX(cs2.created_at) FROM calibration_samples cs2
                       WHERE cs2.source_item_id = si.id
                         AND cs2.processed_at IS NULL) DESC
             LIMIT ?1",
        )
        .context("Failed to prepare sprint candidate query")?;

    let rows = stmt
        .query_map(params![CANDIDATE_POOL as i64], |row| {
            let content: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            Ok(SprintCandidate {
                card: CalibrationSprintCard {
                    source_item_id: row.get(0)?,
                    title: row.get(1)?,
                    snippet: make_snippet(&content),
                    source_type: row.get(3)?,
                    url: row.get(4)?,
                },
                score: row.get::<_, f64>(5)? as f32,
            })
        })
        .context("Failed to query sprint candidates")?;

    let mut out = Vec::new();
    for row in rows {
        match row {
            Ok(c) => out.push(c),
            Err(e) => {
                tracing::warn!(target: "4da::taste_test::sprint", error = %e, "Skipping malformed sprint candidate row");
            }
        }
    }
    Ok(out)
}

/// First ~200 chars of HTML-stripped, whitespace-collapsed content.
/// Reuses the embedding preprocessor so the strip rules stay in one
/// place (`crate::utils::preprocess_content`).
fn make_snippet(content: &str) -> String {
    crate::utils::truncate_utf8(&crate::utils::preprocess_content(content), SNIPPET_CHARS)
}

/// Stratify candidates across score bands and source types.
///
/// Round-robins across the four bands, taking each band's most recent
/// eligible candidate (a candidate is eligible while its source_type
/// is under `per_source_cap`). Bands that run dry simply stop
/// contributing — the others keep filling until `target` cards are
/// chosen or nothing eligible remains. Degrades honestly: fewer than
/// `target` eligible items returns exactly what exists.
pub fn stratify(
    candidates: Vec<SprintCandidate>,
    target: usize,
    per_source_cap: usize,
) -> Vec<CalibrationSprintCard> {
    let mut bands: Vec<Vec<SprintCandidate>> = vec![Vec::new(); BAND_COUNT];
    for c in candidates {
        bands[band_index(c.score)].push(c);
    }
    // Each band is consumed front-to-back (most recent first).
    let mut cursors = [0usize; BAND_COUNT];
    let mut source_counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::with_capacity(target.min(SPRINT_TARGET));

    loop {
        let mut progressed = false;
        for (band, cursor) in bands.iter().zip(cursors.iter_mut()) {
            if out.len() >= target {
                return out;
            }
            // Advance this band's cursor to its next source-eligible card.
            while *cursor < band.len() {
                let candidate = &band[*cursor];
                *cursor += 1;
                let count = source_counts
                    .entry(candidate.card.source_type.clone())
                    .or_insert(0);
                if *count < per_source_cap {
                    *count += 1;
                    out.push(candidate.card.clone());
                    progressed = true;
                    break;
                }
            }
        }
        if !progressed || out.len() >= target {
            return out;
        }
    }
}

/// Fallback pool: scored corpus items WITHOUT requiring a calibration
/// sample. Installs with no LLM advisor configured never stamp
/// `calibration_samples` (the judge path is the only writer), so the
/// primary pool is empty on a fresh no-key install. Explicit labels are
/// still durable ground truth there: they feed precision tracking now
/// and pre-pair with any sample stamped for the item later (feedback
/// labels never expire). Newest scored items first, same exclusions.
fn load_fallback_candidates(conn: &Connection, exclude: &[i64]) -> Result<Vec<SprintCandidate>> {
    let mut stmt = conn
        .prepare(
            "SELECT si.id, si.title, si.content, si.source_type, si.url,
                    COALESCE(si.relevance_score, 0.0)
             FROM source_items si
             WHERE si.relevance_score IS NOT NULL
               AND NOT EXISTS (
                     SELECT 1 FROM feedback f WHERE f.source_item_id = si.id)
               AND TRIM(si.title) <> ''
             ORDER BY si.created_at DESC
             LIMIT ?1",
        )
        .context("Failed to prepare sprint fallback query")?;

    let rows = stmt
        .query_map(params![CANDIDATE_POOL as i64], |row| {
            let content: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            Ok(SprintCandidate {
                card: CalibrationSprintCard {
                    source_item_id: row.get(0)?,
                    title: row.get(1)?,
                    snippet: make_snippet(&content),
                    source_type: row.get(3)?,
                    url: row.get(4)?,
                },
                score: row.get::<_, f64>(5)? as f32,
            })
        })
        .context("Failed to query sprint fallback candidates")?;

    let mut out = Vec::new();
    for row in rows {
        match row {
            Ok(c) => {
                if !exclude.contains(&c.card.source_item_id) {
                    out.push(c);
                }
            }
            Err(e) => {
                tracing::warn!(target: "4da::taste_test::sprint", error = %e, "Skipping malformed fallback candidate row");
            }
        }
    }
    Ok(out)
}

/// Public entry: sample up to [`SPRINT_TARGET`] stratified sprint cards.
///
/// Sample-bearing items lead (their labels pair with the fitter
/// immediately); when they cannot fill the deck — fresh installs, and
/// no-LLM installs where the judge never stamps samples — scored corpus
/// items top it up so the sprint is never artificially empty while
/// real, labelable content exists.
pub fn sprint_items(conn: &Connection) -> Result<Vec<CalibrationSprintCard>> {
    let mut candidates = load_candidates(conn)?;
    if candidates.len() < SPRINT_TARGET {
        let have: Vec<i64> = candidates.iter().map(|c| c.card.source_item_id).collect();
        candidates.extend(load_fallback_candidates(conn, &have)?);
    }
    Ok(stratify(candidates, SPRINT_TARGET, PER_SOURCE_CAP))
}

// ============================================================================
// Status
// ============================================================================

/// Mirror of `calibration_store::path_for`'s layout
/// (`<dir>/<identity_hash>/<task>.json`, components restricted to
/// `[a-zA-Z0-9_-]`), parameterized on the directory for testability.
fn curve_path(calibration_dir: &Path, identity_hash: &str, task: &str) -> std::path::PathBuf {
    let sanitize = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    calibration_dir
        .join(sanitize(identity_hash))
        .join(format!("{}.json", sanitize(task)))
}

/// Whether a persisted calibration curve exists for any (model, task)
/// pair that actually produced calibration samples on THIS install.
///
/// Deliberately NOT a blind directory scan: the live data dir was found
/// to contain an orphaned test-fixture curve (`hash/judge.json`, fit
/// 2026-04-15 by a unit test that wrote through the real RuntimePaths)
/// which would make a scan report "calibration active" to a user whose
/// real models have never fit a curve. Keying on the DB's known model
/// identities grounds the answer in the same store the apply layer
/// reads. Any IO failure reads as "no curve" — matching the store's
/// own pass-through behavior.
pub fn any_curve_for_known_models(conn: &Connection, calibration_dir: &Path) -> Result<bool> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT model_identity_hash, task FROM calibration_samples")
        .context("Failed to prepare model-identity query")?;
    let pairs = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .context("Failed to query model identities")?;
    for pair in pairs.flatten() {
        if curve_path(calibration_dir, &pair.0, &pair.1).exists() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Build the sprint status from a connection plus the curve directory.
/// Split from the command so tests can point at fixture dirs.
pub fn sprint_status(conn: &Connection, calibration_dir: &Path) -> Result<CalibrationSprintStatus> {
    let labeled_total: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT source_item_id) FROM feedback",
            [],
            |r| r.get(0),
        )
        .context("Failed to count labeled items")?;

    Ok(CalibrationSprintStatus {
        labeled_total,
        min_fit_samples: crate::calibration_fitter::MIN_FIT_SAMPLES as u32,
        curve_fitted: any_curve_for_known_models(conn, calibration_dir)?,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Minimal schema the sprint touches — same fixture style as the
    /// calibration_fitter tests.
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                url TEXT,
                relevance_score REAL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE calibration_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                model_identity_hash TEXT NOT NULL DEFAULT 'hash',
                task TEXT NOT NULL DEFAULT 'judge',
                prompt_version TEXT NOT NULL DEFAULT 'p',
                raw_score REAL NOT NULL DEFAULT 0.5,
                confidence REAL NOT NULL DEFAULT 0.5,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                processed_at TEXT
            );
            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER,
                relevant INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn
    }

    fn insert_item(
        conn: &Connection,
        title: &str,
        source_type: &str,
        score: f32,
        content: &str,
    ) -> i64 {
        conn.execute(
            "INSERT INTO source_items (source_type, title, content, relevance_score)
             VALUES (?1, ?2, ?3, ?4)",
            params![source_type, title, content, score as f64],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn insert_unprocessed_sample(conn: &Connection, item_id: i64) {
        conn.execute(
            "INSERT INTO calibration_samples (source_item_id) VALUES (?1)",
            params![item_id],
        )
        .unwrap();
    }

    fn make_candidate(id: i64, source: &str, score: f32) -> SprintCandidate {
        SprintCandidate {
            card: CalibrationSprintCard {
                source_item_id: id,
                title: format!("item {id}"),
                snippet: String::new(),
                source_type: source.to_string(),
                url: None,
            },
            score,
        }
    }

    // -- parse_response -----------------------------------------------------

    #[test]
    fn parse_response_maps_all_known_values() {
        assert_eq!(parse_response("relevant"), Some(SprintResponse::Relevant));
        assert_eq!(
            parse_response("not_relevant"),
            Some(SprintResponse::NotRelevant)
        );
        assert_eq!(parse_response("skip"), Some(SprintResponse::Skip));
        assert_eq!(parse_response("maybe"), None);
        assert_eq!(parse_response(""), None);
    }

    // -- band_index ---------------------------------------------------------

    #[test]
    fn band_index_covers_edges() {
        assert_eq!(band_index(0.0), 0);
        assert_eq!(band_index(0.19), 0);
        assert_eq!(band_index(0.2), 1);
        assert_eq!(band_index(0.44), 1);
        assert_eq!(band_index(0.45), 2);
        assert_eq!(band_index(0.69), 2);
        assert_eq!(band_index(0.7), 3);
        assert_eq!(band_index(1.0), 3);
        // Out-of-range clamps into the edge bands.
        assert_eq!(band_index(-0.5), 0);
        assert_eq!(band_index(2.0), 3);
    }

    // -- stratify -----------------------------------------------------------

    #[test]
    fn stratify_spreads_across_bands_roughly_equally() {
        // 10 candidates in each band, all distinct sources so the
        // source cap never interferes.
        let mut candidates = Vec::new();
        let mut id = 0i64;
        for (b, base) in [(0usize, 0.1f32), (1, 0.3), (2, 0.5), (3, 0.8)] {
            for i in 0..10 {
                id += 1;
                candidates.push(make_candidate(id, &format!("src{b}_{i}"), base));
            }
        }
        let cards = stratify(candidates, 24, 4);
        assert_eq!(cards.len(), 24);
        // Count per band via the id ranges (ids 1-10 band0, 11-20 band1, ...)
        let mut per_band = [0usize; 4];
        for c in &cards {
            per_band[((c.source_item_id - 1) / 10) as usize] += 1;
        }
        for (band, count) in per_band.iter().enumerate() {
            assert_eq!(*count, 6, "band {band} should contribute 24/4 = 6 cards");
        }
    }

    #[test]
    fn stratify_respects_per_source_cap() {
        // 30 candidates, ALL from one source, spread across bands.
        let mut candidates = Vec::new();
        for i in 0..30i64 {
            let score = (i as f32 / 30.0).clamp(0.0, 1.0);
            candidates.push(make_candidate(i + 1, "hackernews", score));
        }
        let cards = stratify(candidates, 24, 4);
        assert_eq!(
            cards.len(),
            4,
            "single-source pool must cap at PER_SOURCE_CAP"
        );
        assert!(cards.iter().all(|c| c.source_type == "hackernews"));
    }

    #[test]
    fn stratify_fills_from_other_bands_when_one_is_empty() {
        // Only two bands populated; target still reached from them.
        let mut candidates = Vec::new();
        for i in 0..8i64 {
            candidates.push(make_candidate(i + 1, &format!("a{i}"), 0.1));
        }
        for i in 0..8i64 {
            candidates.push(make_candidate(i + 100, &format!("b{i}"), 0.9));
        }
        let cards = stratify(candidates, 12, 4);
        assert_eq!(cards.len(), 12);
    }

    #[test]
    fn stratify_degrades_honestly_below_target() {
        let candidates = vec![
            make_candidate(1, "a", 0.1),
            make_candidate(2, "b", 0.5),
            make_candidate(3, "c", 0.9),
        ];
        let cards = stratify(candidates, 24, 4);
        assert_eq!(cards.len(), 3, "show what exists, never pad");
    }

    // -- sprint_items (SQL eligibility) ---------------------------------------

    #[test]
    fn sprint_items_prefers_sample_bearing_items() {
        let conn = test_conn();
        // Item with an unprocessed sample: primary pool, must lead.
        let a = insert_item(&conn, "Eligible item", "hackernews", 0.5, "body");
        insert_unprocessed_sample(&conn, a);
        // Item whose only sample is processed: reachable via fallback.
        let b = insert_item(&conn, "Processed item", "reddit", 0.5, "body");
        conn.execute(
            "INSERT INTO calibration_samples (source_item_id, processed_at)
             VALUES (?1, datetime('now'))",
            params![b],
        )
        .unwrap();
        // Item with no samples at all: reachable via fallback.
        insert_item(&conn, "Sampleless item", "arxiv", 0.5, "body");

        let cards = sprint_items(&conn).unwrap();
        // All three are scored + unlabeled, so all three are labelable;
        // the sample-bearing item leads (its label pairs with the
        // fitter immediately).
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].source_item_id, a);
    }

    #[test]
    fn sprint_items_falls_back_when_no_samples_exist() {
        // Fresh no-LLM install: the judge never stamps
        // calibration_samples, but scored items are still labelable
        // ground truth (cold-start integrity run, 2026-06-12).
        let conn = test_conn();
        insert_item(&conn, "Scored item one", "hackernews", 0.8, "body");
        insert_item(&conn, "Scored item two", "arxiv", 0.1, "body");

        let cards = sprint_items(&conn).unwrap();
        assert_eq!(cards.len(), 2, "fallback serves scored items");
    }

    #[test]
    fn sprint_fallback_still_excludes_labeled_items() {
        let conn = test_conn();
        let a = insert_item(&conn, "Already labeled", "hackernews", 0.5, "body");
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
            params![a],
        )
        .unwrap();
        insert_item(&conn, "Unlabeled", "arxiv", 0.5, "body");

        let cards = sprint_items(&conn).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Unlabeled");
    }

    #[test]
    fn sprint_items_excludes_already_labeled_items() {
        let conn = test_conn();
        let a = insert_item(&conn, "Unlabeled", "hackernews", 0.5, "body");
        insert_unprocessed_sample(&conn, a);
        let b = insert_item(&conn, "Already labeled", "reddit", 0.5, "body");
        insert_unprocessed_sample(&conn, b);
        conn.execute(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (?1, 1)",
            params![b],
        )
        .unwrap();

        let cards = sprint_items(&conn).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].source_item_id, a);
    }

    #[test]
    fn sprint_items_excludes_blank_titles() {
        let conn = test_conn();
        let a = insert_item(&conn, "   ", "hackernews", 0.5, "body");
        insert_unprocessed_sample(&conn, a);
        let b = insert_item(&conn, "Real title", "reddit", 0.5, "body");
        insert_unprocessed_sample(&conn, b);

        let cards = sprint_items(&conn).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Real title");
    }

    #[test]
    fn sprint_items_strips_html_in_snippet() {
        let conn = test_conn();
        let a = insert_item(
            &conn,
            "HTML item",
            "rss",
            0.5,
            "<p>Hello <b>world</b> &amp; beyond</p>",
        );
        insert_unprocessed_sample(&conn, a);

        let cards = sprint_items(&conn).unwrap();
        assert_eq!(cards.len(), 1);
        assert!(!cards[0].snippet.contains('<'));
        assert!(cards[0].snippet.contains("Hello"));
        assert!(cards[0].snippet.contains("& beyond"));
    }

    #[test]
    fn sprint_items_returns_empty_when_nothing_eligible() {
        let conn = test_conn();
        let cards = sprint_items(&conn).unwrap();
        assert!(cards.is_empty(), "no fake cards on an empty corpus");
    }

    // -- status ---------------------------------------------------------------

    #[test]
    fn status_counts_distinct_labeled_items() {
        let conn = test_conn();
        let dir = tempfile::tempdir().unwrap();
        conn.execute_batch(
            "INSERT INTO feedback (source_item_id, relevant) VALUES (1, 1);
             INSERT INTO feedback (source_item_id, relevant) VALUES (1, 0);
             INSERT INTO feedback (source_item_id, relevant) VALUES (2, 1);",
        )
        .unwrap();

        let status = sprint_status(&conn, dir.path()).unwrap();
        assert_eq!(
            status.labeled_total, 2,
            "repeat labels on one item count once"
        );
        assert_eq!(
            status.min_fit_samples,
            crate::calibration_fitter::MIN_FIT_SAMPLES as u32,
            "floor must come from the fitter const, never a local copy"
        );
        assert!(!status.curve_fitted);
    }

    #[test]
    fn status_detects_persisted_curve_for_known_model() {
        let conn = test_conn();
        let dir = tempfile::tempdir().unwrap();
        // A sample establishes 'abc123'/'judge' as a known pair...
        conn.execute(
            "INSERT INTO calibration_samples (source_item_id, model_identity_hash, task)
             VALUES (1, 'abc123', 'judge')",
            [],
        )
        .unwrap();
        // ...and its curve file exists in the store layout.
        let hash_dir = dir.path().join("abc123");
        std::fs::create_dir_all(&hash_dir).unwrap();
        std::fs::write(hash_dir.join("judge.json"), b"{}").unwrap();

        let status = sprint_status(&conn, dir.path()).unwrap();
        assert!(status.curve_fitted);
    }

    #[test]
    fn status_ignores_orphaned_curve_files() {
        // Regression guard for the real incident: a stale test-fixture
        // curve (hash/judge.json) on disk with NO matching model in
        // calibration_samples must NOT report calibration as active.
        let conn = test_conn();
        let dir = tempfile::tempdir().unwrap();
        conn.execute(
            "INSERT INTO calibration_samples (source_item_id, model_identity_hash, task)
             VALUES (1, 'realsha256identity', 'judge')",
            [],
        )
        .unwrap();
        let orphan_dir = dir.path().join("hash");
        std::fs::create_dir_all(&orphan_dir).unwrap();
        std::fs::write(orphan_dir.join("judge.json"), b"{}").unwrap();

        let status = sprint_status(&conn, dir.path()).unwrap();
        assert!(
            !status.curve_fitted,
            "orphaned curve files must not fake an active calibration"
        );
    }

    #[test]
    fn status_zero_on_empty_feedback() {
        let conn = test_conn();
        let dir = tempfile::tempdir().unwrap();
        let status = sprint_status(&conn, dir.path()).unwrap();
        assert_eq!(status.labeled_total, 0);
    }

    #[test]
    fn ts_contract_is_camel_case() {
        let card = CalibrationSprintCard {
            source_item_id: 7,
            title: "T".into(),
            snippet: "S".into(),
            source_type: "hackernews".into(),
            url: Some("https://example.com".into()),
        };
        let json = serde_json::to_value(&card).unwrap();
        assert_eq!(json["sourceItemId"], 7);
        assert_eq!(json["sourceType"], "hackernews");

        let status = CalibrationSprintStatus {
            labeled_total: 3,
            min_fit_samples: 50,
            curve_fitted: false,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["labeledTotal"], 3);
        assert_eq!(json["minFitSamples"], 50);
        assert_eq!(json["curveFitted"], false);
    }
}

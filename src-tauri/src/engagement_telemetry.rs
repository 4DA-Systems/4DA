// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.

//! Engagement Telemetry — bridges user interactions to the stability detector.
//!
//! Every click, save, dismiss, and explicit feedback event flows through here
//! to record evidence for learned preferences. This is the wire that makes
//! "gets sharper every day" real: interactions become evidence, evidence
//! accumulates into facets, facets influence scoring.

use rusqlite::{params, Connection};
use tracing::debug;

use crate::stability_detector::{self, CueFamily, FacetClass};

/// Metadata about a source item, used to extract preference signals.
#[derive(Debug, Clone)]
pub struct ItemContext {
    pub source_type: String,
    pub title: String,
    pub tags: Option<String>,
}

/// Look up minimal item metadata needed for preference recording.
pub fn lookup_item_context(conn: &Connection, source_item_id: i64) -> Option<ItemContext> {
    conn.query_row(
        "SELECT source_type, title, tags FROM source_items WHERE id = ?1",
        params![source_item_id],
        |row| {
            Ok(ItemContext {
                source_type: row.get(0)?,
                title: row.get(1)?,
                tags: row.get::<_, Option<String>>(2)?,
            })
        },
    )
    .ok()
}

// ============================================================================
// Interaction → Evidence
// ============================================================================

/// Record preference evidence when a user clicks (opens) content.
/// Moderate signal: they were interested enough to read it.
pub fn on_click(conn: &Connection, source_item_id: i64) {
    let Some(ctx) = lookup_item_context(conn, source_item_id) else {
        return;
    };

    stability_detector::record_evidence(
        conn,
        FacetClass::SourcePref,
        &ctx.source_type,
        "engaged",
        CueFamily::Behavioral,
        "click",
        0.5,
    );

    for tag in extract_tags(&ctx.tags) {
        stability_detector::record_evidence(
            conn,
            FacetClass::TopicAffinity,
            tag,
            "clicked",
            CueFamily::Behavioral,
            "click",
            0.4,
        );
    }

    debug!(target: "4da::telemetry", source = %ctx.source_type, item = source_item_id, "Click evidence recorded");
}

/// Record preference evidence when a user saves content.
/// Strong signal: explicit positive action.
pub fn on_save(conn: &Connection, source_item_id: i64) {
    let Some(ctx) = lookup_item_context(conn, source_item_id) else {
        return;
    };

    stability_detector::record_evidence(
        conn,
        FacetClass::SourcePref,
        &ctx.source_type,
        "valued",
        CueFamily::Behavioral,
        "save",
        0.8,
    );

    for tag in extract_tags(&ctx.tags) {
        stability_detector::record_evidence(
            conn,
            FacetClass::TopicAffinity,
            tag,
            "saved",
            CueFamily::Behavioral,
            "save",
            0.7,
        );
    }

    stability_detector::record_evidence(
        conn,
        FacetClass::Interest,
        &normalize_title_topic(&ctx.title),
        "saved",
        CueFamily::Behavioral,
        "save",
        0.6,
    );

    debug!(target: "4da::telemetry", source = %ctx.source_type, item = source_item_id, "Save evidence recorded");
}

/// Record preference evidence when a user dismisses content.
/// Negative signal: they explicitly rejected it.
pub fn on_dismiss(conn: &Connection, source_item_id: i64, category: Option<&str>) {
    let Some(ctx) = lookup_item_context(conn, source_item_id) else {
        return;
    };

    if let Some(cat) = category {
        stability_detector::record_evidence(
            conn,
            FacetClass::Veto,
            cat,
            "dismissed",
            CueFamily::Behavioral,
            "dismiss_category",
            0.6,
        );
    }

    for tag in extract_tags(&ctx.tags) {
        stability_detector::record_evidence(
            conn,
            FacetClass::TopicAffinity,
            tag,
            "dismissed",
            CueFamily::Behavioral,
            "dismiss",
            -0.3_f64.abs(), // negative affinity recorded as low confidence
        );
    }

    debug!(target: "4da::telemetry", source = %ctx.source_type, item = source_item_id, "Dismiss evidence recorded");
}

// ============================================================================
// Explicit Feedback → Evidence
// ============================================================================

/// Record evidence from explicit relevance feedback (thumbs up/down).
/// Note: `ace_commands::record_item_feedback` has its own stability wiring.
/// This function exists for any other feedback paths that bypass that command.
#[allow(dead_code)] // REMOVE BY 2026-08-01 — will be wired when feedback paths unify
pub fn on_feedback(conn: &Connection, source_item_id: i64, relevant: bool) {
    let Some(ctx) = lookup_item_context(conn, source_item_id) else {
        return;
    };

    let confidence = if relevant { 0.9 } else { 0.1 };

    stability_detector::record_evidence(
        conn,
        FacetClass::SourcePref,
        &ctx.source_type,
        if relevant { "endorsed" } else { "rejected" },
        CueFamily::Explicit,
        "feedback",
        confidence,
    );

    for tag in extract_tags(&ctx.tags) {
        if relevant {
            stability_detector::record_evidence(
                conn,
                FacetClass::TopicAffinity,
                tag,
                "endorsed",
                CueFamily::Explicit,
                "feedback_positive",
                0.9,
            );
        } else {
            stability_detector::record_evidence(
                conn,
                FacetClass::Veto,
                tag,
                "rejected",
                CueFamily::Explicit,
                "feedback_negative",
                0.7,
            );
        }
    }

    debug!(target: "4da::telemetry", relevant, source = %ctx.source_type, item = source_item_id, "Feedback evidence recorded");
}

// ============================================================================
// Blind Spot Dismissal → Evidence
// ============================================================================

/// Record evidence when a user dismisses a blind spot.
/// This tells us the topic is not actually a gap — either known or irrelevant.
pub fn on_blind_spot_dismiss(conn: &Connection, topic: &str) {
    stability_detector::record_evidence(
        conn,
        FacetClass::Veto,
        topic,
        "blind_spot_dismissed",
        CueFamily::Explicit,
        "blind_spot_dismiss",
        0.8,
    );

    debug!(target: "4da::telemetry", topic, "Blind spot dismissal evidence recorded");
}

// ============================================================================
// Post-Analysis Rebuild
// ============================================================================

/// Rebuild stability scores after new evidence has accumulated.
/// Call this after analysis cycles to update facet states.
pub fn rebuild_if_needed(conn: &Connection) {
    let evidence_since_last: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM facet_evidence
             WHERE observed_at > COALESCE(
                 (SELECT value FROM kv WHERE key = 'stability_last_rebuild'), 0
             )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if evidence_since_last < 3 {
        return;
    }

    let updated = stability_detector::rebuild_all(conn);

    let now = stability_detector::now_unix_pub();
    let _ = conn.execute(
        "INSERT OR REPLACE INTO kv (key, value) VALUES ('stability_last_rebuild', ?1)",
        params![now.to_string()],
    );

    if updated > 0 {
        debug!(target: "4da::telemetry", updated, "Stability rebuild completed after new evidence");
    }
}

// ============================================================================
// Implicit Dismissal (dwell without interaction)
// ============================================================================

/// Record a weak negative signal when an item was visible long enough to
/// read (>5s) but the user never clicked, saved, or interacted with it.
pub fn on_implicit_skip(conn: &Connection, source_item_id: i64, dwell_seconds: f32) {
    if dwell_seconds < 5.0 {
        return;
    }

    let Some(ctx) = lookup_item_context(conn, source_item_id) else {
        return;
    };

    let strength = if dwell_seconds > 15.0 { 0.15 } else { 0.10 };

    for tag in extract_tags(&ctx.tags) {
        stability_detector::record_evidence(
            conn,
            FacetClass::TopicAffinity,
            tag,
            "skipped",
            CueFamily::Behavioral,
            "implicit_skip",
            strength,
        );
    }

    debug!(
        target: "4da::telemetry",
        source = %ctx.source_type,
        item = source_item_id,
        dwell_seconds,
        "Implicit skip evidence recorded"
    );
}

// ============================================================================
// Helpers
// ============================================================================

fn extract_tags(tags: &Option<String>) -> Vec<&str> {
    match tags {
        Some(t) if !t.is_empty() => t
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn normalize_title_topic(title: &str) -> String {
    let lower = title.to_lowercase();
    let words: Vec<&str> = lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .take(3)
        .collect();
    words.join("_")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE source_items (
                id INTEGER PRIMARY KEY,
                source_type TEXT NOT NULL,
                title TEXT NOT NULL,
                tags TEXT,
                url TEXT,
                content TEXT,
                content_hash TEXT,
                embedding BLOB DEFAULT x'',
                created_at TEXT DEFAULT (datetime('now')),
                last_seen TEXT DEFAULT (datetime('now')),
                detected_lang TEXT DEFAULT 'en',
                feed_origin TEXT,
                embedding_status TEXT
            );
            CREATE TABLE learned_facets (
                facet_id TEXT PRIMARY KEY,
                class TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                stability REAL NOT NULL DEFAULT 0.0,
                state TEXT NOT NULL DEFAULT 'candidate',
                user_state TEXT NOT NULL DEFAULT 'auto',
                evidence_count INTEGER NOT NULL DEFAULT 0,
                first_seen_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL,
                UNIQUE(class, key)
            );
            CREATE TABLE facet_evidence (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                facet_id TEXT NOT NULL,
                cue_family TEXT NOT NULL,
                evidence_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                observed_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                value TEXT
            );",
        )
        .unwrap();
        conn
    }

    fn insert_test_item(conn: &Connection, id: i64, source_type: &str, title: &str, tags: &str) {
        conn.execute(
            "INSERT INTO source_items (id, source_type, title, tags) VALUES (?1, ?2, ?3, ?4)",
            params![id, source_type, title, tags],
        )
        .unwrap();
    }

    #[test]
    fn click_records_source_and_topic_evidence() {
        let conn = setup_db();
        insert_test_item(&conn, 1, "hackernews", "Rust async patterns", "rust,async");

        on_click(&conn, 1);

        let source_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM facet_evidence WHERE evidence_type = 'click'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // 1 source_pref + 2 topic_affinity (rust, async)
        assert_eq!(source_count, 3);
    }

    #[test]
    fn save_records_strong_evidence() {
        let conn = setup_db();
        insert_test_item(
            &conn,
            2,
            "arxiv",
            "Transformer architecture",
            "ml,transformers",
        );

        on_save(&conn, 2);

        let facets: i64 = conn
            .query_row("SELECT COUNT(*) FROM learned_facets", [], |row| row.get(0))
            .unwrap();
        // source_pref:arxiv + topic_affinity:ml + topic_affinity:transformers
        // (interest from title may be filtered by is_display_worthy gate)
        assert!(facets >= 3);
    }

    #[test]
    fn dismiss_with_category_records_veto() {
        let conn = setup_db();
        insert_test_item(&conn, 3, "reddit", "Crypto trading bot", "crypto,trading");

        on_dismiss(&conn, 3, Some("crypto"));

        let veto_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM learned_facets WHERE class = 'veto'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(veto_count >= 1);
    }

    #[test]
    fn feedback_positive_records_explicit_evidence() {
        let conn = setup_db();
        insert_test_item(&conn, 4, "github", "New TypeScript compiler", "typescript");

        on_feedback(&conn, 4, true);

        let explicit_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM facet_evidence WHERE cue_family = 'explicit'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(explicit_count >= 2); // source + topic
    }

    #[test]
    fn missing_item_is_silent() {
        let conn = setup_db();
        on_click(&conn, 999);
        on_save(&conn, 999);
        on_dismiss(&conn, 999, None);
        on_feedback(&conn, 999, true);
        // No panic, no evidence recorded
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM facet_evidence", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn rebuild_skips_when_insufficient_evidence() {
        let conn = setup_db();
        rebuild_if_needed(&conn);
        // Should not panic — just returns without doing anything
    }

    #[test]
    fn implicit_skip_records_weak_evidence() {
        let conn = setup_db();
        insert_test_item(
            &conn,
            5,
            "hackernews",
            "WebAssembly runtime",
            "wasm,runtime",
        );

        on_implicit_skip(&conn, 5, 10.0);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM facet_evidence WHERE evidence_type = 'implicit_skip'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2); // wasm + runtime

        let max_conf: f64 = conn
            .query_row(
                "SELECT MAX(confidence) FROM facet_evidence WHERE evidence_type = 'implicit_skip'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(max_conf <= 0.15, "Implicit skip should use weak confidence");
    }

    #[test]
    fn implicit_skip_ignored_below_threshold() {
        let conn = setup_db();
        insert_test_item(&conn, 6, "reddit", "Quick glance", "rust");

        on_implicit_skip(&conn, 6, 3.0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM facet_evidence", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn implicit_skip_stronger_for_long_dwell() {
        let conn = setup_db();
        insert_test_item(&conn, 7, "arxiv", "Long visible item", "ml");

        on_implicit_skip(&conn, 7, 20.0);

        let conf: f64 = conn
            .query_row(
                "SELECT confidence FROM facet_evidence WHERE evidence_type = 'implicit_skip'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            (conf - 0.15).abs() < 0.01,
            "Long dwell should use 0.15 strength"
        );
    }
}

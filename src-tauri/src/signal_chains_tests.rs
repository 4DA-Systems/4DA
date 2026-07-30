// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for signal_chains — EvidenceItem conversion (Intelligence Reconciliation,
//! Phase 5) and the grounding policy (chain_policy). Split out of signal_chains.rs to
//! keep the implementation file under the size limit; included via `#[path]` so these
//! remain a child module with access to the parent's private items.

use super::*;
use rusqlite::{params, Connection};

fn chain_detection_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE source_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT DEFAULT 'test',
            source_id TEXT DEFAULT '',
            title TEXT DEFAULT '',
            content TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now')),
            published_at TEXT,
            last_seen TEXT DEFAULT (datetime('now')),
            tags TEXT,
            relevance_score REAL,
            embedding_status TEXT DEFAULT 'complete'
        );
        CREATE TABLE user_dependencies (
            package_name TEXT NOT NULL,
            ecosystem TEXT DEFAULT '',
            is_dev INTEGER DEFAULT 0,
            is_direct INTEGER DEFAULT 1
        );
        CREATE TABLE project_dependencies (
            package_name TEXT NOT NULL,
            language TEXT DEFAULT '',
            is_dev INTEGER DEFAULT 0,
            is_direct INTEGER DEFAULT 1
        );
        CREATE TABLE temporal_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            subject TEXT NOT NULL,
            data JSON NOT NULL,
            embedding BLOB,
            source_item_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        );",
    )
    .expect("create chain test schema");
    conn
}

fn insert_chain_test_item(
    conn: &Connection,
    source_id: &str,
    source_type: &str,
    title: &str,
    content: &str,
    time_modifier: &str,
    relevance_score: f64,
) {
    conn.execute(
        "INSERT INTO source_items (
            source_type, source_id, title, content, created_at, relevance_score, embedding_status
         ) VALUES (?1, ?2, ?3, ?4, datetime('now', ?5), ?6, 'complete')",
        params![
            source_type,
            source_id,
            title,
            content,
            time_modifier,
            relevance_score
        ],
    )
    .expect("insert chain test item");
}

// ------------------------------------------------------------------------
// Grounding policy (chain_policy) — keyword-inferred severity must not mint a
// critical alert for a topic the user does not actually depend on.
// ------------------------------------------------------------------------

#[test]
fn detect_chains_samples_across_days_not_only_latest_two_hundred_items() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name) VALUES ('tokio')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "tokio-yesterday",
        "hackernews",
        "Tokio patch released after vulnerability report",
        "tokio async runtime release notes",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "tokio-today",
        "hackernews",
        "CVE-2026-0001 vulnerability in Tokio runtime",
        "tokio security advisory",
        "-10 minutes",
        1.0,
    );

    // This mirrors the live failure: the old newest-200 query would look only
    // at this same-day burst and reject every topic because no candidate could
    // span two days.
    for i in 0..220 {
        insert_chain_test_item(
            &conn,
            &format!("noise-{i}"),
            "hackernews",
            &format!("Fresh AI noise item {i}"),
            "same-day burst",
            "-1 minutes",
            0.1,
        );
    }

    let chains = detect_chains(&conn).expect("detect chains");
    let tokio = chains
        .iter()
        .find(|c| c.verified_dep.as_deref() == Some("tokio"))
        .expect("tokio chain should survive a same-day burst");

    assert_eq!(tokio.links.len(), 2);
    assert_eq!(tokio.overall_priority, "critical");
    assert!(
        tokio
            .links
            .iter()
            .any(|link| link.signal_type == "security_alert"),
        "chain should preserve the security link"
    );
}

#[test]
fn strict_proof_dependency_topic_is_not_verified_without_ecosystem_context() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name, ecosystem) VALUES ('image', 'rust')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "image-yesterday",
        "hackernews",
        "Image vulnerability report spreads",
        "photography pipeline security discussion",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "image-two-days",
        "lemmy",
        "Image incident follow-up",
        "generic photography pipeline discussion",
        "-2 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "image-three-days",
        "mastodon",
        "Image media-processing analysis",
        "photo app reliability discussion",
        "-3 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "image-today",
        "reddit",
        "Image exploit analysis",
        "media processing incident",
        "-10 minutes",
        1.0,
    );

    let chains = detect_chains(&conn).expect("detect chains");
    assert!(
        chains
            .iter()
            .all(|c| c.verified_dep.as_deref() != Some("image")),
        "ambiguous image mentions without Rust/crate context must not verify the dependency"
    );

    let image = chains
        .iter()
        .find(|c| c.chain_name.starts_with("image signal chain"))
        .expect("well-corroborated image awareness chain should still exist");

    assert_eq!(image.verified_dep, None);
    assert_eq!(image.overall_priority, "watch");
    assert!(
        image.confidence <= UNGROUNDED_CONFIDENCE_CAP + f64::EPSILON,
        "ambiguous package name without ecosystem proof must not enter the grounded band"
    );
}

#[test]
fn strict_proof_dependency_topic_can_be_verified_with_ecosystem_context() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name, ecosystem) VALUES ('image', 'rust')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "image-crate-yesterday",
        "hackernews",
        "Image crate patch released",
        "rust package release notes",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "image-crate-today",
        "reddit",
        "CVE-2026-0002 vulnerability in Image crate",
        "cargo security advisory",
        "-10 minutes",
        1.0,
    );

    let chains = detect_chains(&conn).expect("detect chains");
    let image = chains
        .iter()
        .find(|c| c.verified_dep.as_deref() == Some("image"))
        .expect("image crate chain should be verified with rust ecosystem proof");

    assert_eq!(image.overall_priority, "critical");
    assert!(image.confidence > UNGROUNDED_CONFIDENCE_CAP);
}

#[test]
fn verified_dep_chain_filters_ungrounded_security_link() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name, ecosystem) VALUES ('image', 'rust')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "image-maintenance",
        "hackernews",
        "Image crate maintenance update",
        "rust package maintenance notes",
        "-2 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "image-docs",
        "reddit",
        "Image crate docs refreshed",
        "cargo module documentation",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "image-unrelated-exploit",
        "mastodon",
        "Image exploit analysis",
        "media processing incident with no package context",
        "-10 minutes",
        1.0,
    );

    let chains = detect_chains(&conn).expect("detect chains");
    let image = chains
        .iter()
        .find(|c| c.verified_dep.as_deref() == Some("image"))
        .expect("image chain should be dependency-verified by crate links");

    assert_eq!(image.overall_priority, "watch");
    assert_eq!(image.links.len(), 2);
    assert!(
        image
            .links
            .iter()
            .all(|link| link.signal_type != "security_alert"),
        "raw security activity without grounded dependency evidence should not appear in a verified dependency chain"
    );
}

#[test]
fn verified_dep_chain_requires_two_grounded_items_across_days() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name, ecosystem) VALUES ('express', 'javascript')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "express-package",
        "npm_registry",
        "Express package 5.0 released",
        "express package update on npm",
        "-3 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "defense-express",
        "rss",
        "Defense Express reports new vehicle",
        "war news article",
        "-2 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "plain-express",
        "reddit",
        "Developers express concern about CI",
        "ordinary english verb use",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "another-express",
        "hackernews",
        "People express opinions about software",
        "ordinary english verb use",
        "-10 minutes",
        1.0,
    );

    let chains = detect_chains(&conn).expect("detect chains");
    assert!(
        chains
            .iter()
            .all(|c| c.verified_dep.as_deref() != Some("express")),
        "one grounded express package hit plus unrelated same-word items must not verify a chain"
    );
}

#[test]
fn verified_dep_chain_displays_only_grounded_links() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name, ecosystem) VALUES ('express', 'javascript')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "defense-express",
        "rss",
        "Defense Express reports new vehicle",
        "war news article",
        "-4 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "express-package-release",
        "npm_registry",
        "Express package 5.0 released",
        "express package update on npm",
        "-3 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "plain-express",
        "reddit",
        "Developers express concern about CI",
        "ordinary english verb use",
        "-2 days",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "express-package-security",
        "security",
        "Express npm package security advisory",
        "express package vulnerability on npm",
        "-1 day",
        1.0,
    );

    let chains = detect_chains(&conn).expect("detect chains");
    let express = chains
        .iter()
        .find(|c| c.verified_dep.as_deref() == Some("express"))
        .expect("express should verify with two grounded package links");

    assert_eq!(express.links.len(), 2);
    assert!(express
        .links
        .iter()
        .all(|link| link.title.to_lowercase().contains("package")));
    assert!(!express
        .links
        .iter()
        .any(|link| link.title.contains("Defense Express")));
}

#[test]
fn critical_verified_chain_keeps_security_link_when_truncated() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name, ecosystem) VALUES ('next', 'javascript')",
        [],
    )
    .expect("insert dependency");

    for i in 0..5 {
        insert_chain_test_item(
            &conn,
            &format!("next-learning-{i}"),
            "blog",
            &format!("Next.js architecture note {i}"),
            "next.js package patterns",
            &format!("-{} days", 6 - i),
            1.0,
        );
    }
    insert_chain_test_item(
        &conn,
        "next-security",
        "security",
        "GHSA-0000 Next.js package security advisory",
        "next.js package vulnerability on npm",
        "-10 minutes",
        1.0,
    );

    let chains = detect_chains(&conn).expect("detect chains");
    let next = chains
        .iter()
        .find(|c| c.verified_dep.as_deref() == Some("next"))
        .expect("next chain should verify");

    assert_eq!(next.overall_priority, "critical");
    assert_eq!(next.links.len(), 5);
    assert!(
        next.links
            .iter()
            .any(|link| link.signal_type == "security_alert"),
        "critical chain display must retain the grounded security link"
    );
}

#[test]
fn detect_and_record_chains_persists_temporal_signal_chain_rows() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO user_dependencies (package_name) VALUES ('tokio')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "tokio-yesterday",
        "hackernews",
        "Tokio patch released",
        "tokio runtime release notes",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "tokio-today",
        "security",
        "CVE-2026-0001 vulnerability in Tokio runtime",
        "tokio security advisory",
        "-10 minutes",
        1.0,
    );

    let chains = detect_and_record_chains(&conn).expect("detect and persist");
    assert_eq!(chains.len(), 1);

    let (subject, data, source_item_id, expires_at): (String, String, Option<i64>, Option<String>) =
        conn.query_row(
            "SELECT subject, data, source_item_id, expires_at
             FROM temporal_events
             WHERE event_type = 'signal_chain'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("persisted signal chain row");

    assert!(subject.starts_with("tokio signal chain"));
    assert!(source_item_id.is_some());
    assert!(expires_at.is_some());

    let payload: serde_json::Value = serde_json::from_str(&data).expect("signal chain json");
    assert_eq!(payload["schema_version"], 1);
    assert_eq!(payload["chain"]["verified_dep"], "tokio");
    assert_eq!(payload["prediction"]["phase"], "nascent");
    assert_eq!(payload["source_item_ids"].as_array().unwrap().len(), 2);
}

#[test]
fn detect_and_record_chains_replaces_stale_signal_chain_snapshot() {
    let conn = chain_detection_db();
    conn.execute(
        "INSERT INTO temporal_events (event_type, subject, data)
         VALUES ('signal_chain', 'stale chain', '{}')",
        [],
    )
    .expect("insert stale snapshot");
    conn.execute(
        "INSERT INTO user_dependencies (package_name) VALUES ('tokio')",
        [],
    )
    .expect("insert dependency");

    insert_chain_test_item(
        &conn,
        "tokio-yesterday",
        "hackernews",
        "Tokio patch released",
        "tokio runtime release notes",
        "-1 day",
        1.0,
    );
    insert_chain_test_item(
        &conn,
        "tokio-today",
        "security",
        "CVE-2026-0001 vulnerability in Tokio runtime",
        "tokio security advisory",
        "-10 minutes",
        1.0,
    );

    detect_and_record_chains(&conn).expect("detect and persist");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM temporal_events WHERE event_type = 'signal_chain'",
            [],
            |row| row.get(0),
        )
        .expect("signal chain count");
    let stale_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM temporal_events WHERE subject = 'stale chain'",
            [],
            |row| row.get(0),
        )
        .expect("stale count");

    assert_eq!(count, 1);
    assert_eq!(stale_count, 0);
}

#[test]
fn ungrounded_keyword_security_cannot_be_critical() {
    // Topic is NOT an installed dep (dep_match = 0). Even a fully-corroborated
    // "security" chain (5 links) must stay awareness-only, never "critical".
    let p = chain_policy(true, false, 0.0, 5);
    assert_eq!(
        p.priority, "watch",
        "ungrounded keyword-security chain must not be critical"
    );
}

#[test]
fn ungrounded_breaking_cannot_be_alert() {
    let p = chain_policy(false, true, 0.0, 5);
    assert_eq!(p.priority, "watch");
}

#[test]
fn grounded_security_is_critical() {
    // Same security signal, but now the topic IS an installed dependency.
    let p = chain_policy(true, false, 0.6, 3);
    assert_eq!(p.priority, "critical");
}

#[test]
fn grounded_breaking_is_alert() {
    let p = chain_policy(false, true, 0.6, 3);
    assert_eq!(p.priority, "alert");
}

#[test]
fn grounded_thin_vs_corroborated_non_security() {
    // Installed dep, no security/breaking: 3+ links → advisory, fewer → watch.
    assert_eq!(chain_policy(false, false, 0.6, 3).priority, "advisory");
    assert_eq!(chain_policy(false, false, 0.6, 2).priority, "watch");
}

#[test]
fn ungrounded_confidence_capped_below_grounded_band() {
    // The worst pre-fix case: a 2-link "security" chain on a non-dep topic used to
    // surface at "critical" with confidence ~0.32. Confidence is now capped, and the
    // cap sits strictly below the floor any grounded chain can reach.
    let ungrounded = chain_policy(true, false, 0.0, 5);
    assert!(
        ungrounded.confidence <= UNGROUNDED_CONFIDENCE_CAP + f64::EPSILON,
        "ungrounded confidence {} exceeded cap {}",
        ungrounded.confidence,
        UNGROUNDED_CONFIDENCE_CAP
    );

    // Weakest possible grounded chain (min dep_match 0.5, 2 links, learning severity).
    let grounded_floor = chain_policy(false, false, 0.5, 2);
    assert!(
        grounded_floor.confidence > UNGROUNDED_CONFIDENCE_CAP,
        "grounded floor {} should exceed ungrounded cap {}",
        grounded_floor.confidence,
        UNGROUNDED_CONFIDENCE_CAP
    );
}

#[test]
fn grounded_chains_retain_dependency_weighted_confidence() {
    // More dependency matches → higher confidence (dep relevance is the 50% term).
    let one = chain_policy(false, false, 0.5, 3).confidence;
    let many = chain_policy(false, false, 0.9, 3).confidence;
    assert!(many > one);
}

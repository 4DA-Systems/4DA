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

/// Anchor for every fixture timestamp: **noon yesterday**, not `now` — and the
/// SINGLE definition of it, shared by the fixtures and by the test that proves it
/// is clock-independent.
///
/// Chain detection buckets items by calendar day (`DATE(signal_at)` in the
/// candidate query, `ts.get(..10)` in the multi-day gate), so a fixture that
/// offsets from `now` silently changes shape depending on the wall clock. With
/// `now` as the anchor, `-10 minutes` and `-1 day` land on DIFFERENT days for
/// most of the day but the SAME day whenever the suite runs in the first ten
/// minutes after UTC midnight — at 00:02 UTC both resolve to yesterday, the
/// topic collapses to one distinct date, and the multi-day gate rejects it.
///
/// That is not hypothetical: it took out four tests in a merge-queue run at
/// 2026-09-01T00:02Z and dequeued a frontend-only PR that could not have caused
/// it. For this fleet the window is 10:00-10:10 AEST — the middle of a working
/// day, not a quiet night.
///
/// Anchoring to a fixed time-of-day makes every offset land on a deterministic
/// calendar date whatever the clock says. Yesterday rather than today so that
/// no fixture is stamped in the future, and noon so that a whole-day offset
/// cannot drift across a boundary. The largest offset in use is `-4 days`,
/// giving 5 days from now — still inside the 7-day candidate window.
///
/// `base` is a SQL expression standing in for "now" — `'now'` for real fixtures,
/// an explicit bound timestamp for the test. Both callers go through here on
/// purpose: if someone reverts the anchor to `datetime({base}, {offset})`, the
/// test breaks too. A regression test that re-implements the thing it guards
/// guards nothing.
fn fixture_timestamp_sql(base: &str, offset_param: &str) -> String {
    format!("datetime(date({base}), '-1 day', '+12 hours', {offset_param})")
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
    let sql = format!(
        "INSERT INTO source_items (
            source_type, source_id, title, content, created_at, relevance_score, embedding_status
         ) VALUES (?1, ?2, ?3, ?4, {}, ?6, 'complete')",
        fixture_timestamp_sql("'now'", "?5")
    );
    conn.execute(
        &sql,
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

/// The anchor must separate days at EVERY hour — including the ten minutes after
/// UTC midnight that hid the original bug.
///
/// The sibling test below exercises the anchor at whatever time the suite happens
/// to run, which is not enough on its own: the OLD `datetime('now', ?)` anchor also
/// satisfies it for 1430 minutes out of every 1440. A guard that only fires during
/// the 0.7% of the day when the bug is visible is the same blind spot that let this
/// reach the merge queue in the first place. This test pins the shared
/// [`fixture_timestamp_sql`] against explicit base timestamps, so a revert fails it
/// at any hour of any day.
#[test]
fn the_anchor_separates_days_at_every_hour_including_just_after_midnight() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    let sql = fixture_timestamp_sql("?1", "?2");
    let day_of = |base: &str, offset: &str| -> String {
        conn.query_row(&format!("SELECT DATE({sql})"), params![base, offset], |r| {
            r.get::<_, String>(0)
        })
        .expect("evaluate the fixture anchor")
    };

    for base in [
        "2026-09-01 00:00:00", // the boundary itself
        "2026-09-01 00:02:00", // the minute CI actually failed
        "2026-09-01 00:09:59", // last second of the old anchor's blind spot
        "2026-09-01 00:10:01", // first second outside it
        "2026-09-01 12:00:00", // the ordinary case that always passed
        "2026-08-31 23:59:59", // the far side of the boundary
        "2026-03-01 00:05:00", // a month boundary, inside the window
    ] {
        assert_ne!(
            day_of(base, "-10 minutes"),
            day_of(base, "-1 day"),
            "with the clock at {base} the anchor put a sub-hour offset and a -1 day \
             offset on the SAME calendar day. That collapse is what failed four \
             signal_chains tests at 2026-09-01T00:02Z and dequeued a frontend-only PR."
        );
    }

    // The same-day pairing the fixtures rely on must still hold at those hours.
    for base in ["2026-09-01 00:02:00", "2026-09-01 12:00:00"] {
        assert_eq!(
            day_of(base, "-10 minutes"),
            day_of(base, "-1 minutes"),
            "two sub-hour offsets must share a calendar day at {base}"
        );
    }
}

/// The fixture anchor must make day bucketing independent of the wall clock.
///
/// Guards the regression directly: assert that the two offsets which collided at
/// 00:02 UTC (`-10 minutes` and `-1 day`) resolve to two distinct calendar dates,
/// and that a same-day pair still shares one. Without the anchor this passes for
/// 1430 minutes a day and fails for 10, which is exactly why it went unnoticed.
#[test]
fn fixture_offsets_land_on_stable_calendar_days() {
    let conn = chain_detection_db();
    for (id, offset) in [
        ("recent", "-10 minutes"),
        ("also-recent", "-1 minutes"),
        ("yesterday", "-1 day"),
        ("older", "-4 days"),
    ] {
        insert_chain_test_item(&conn, id, "hackernews", "t", "c", offset, 1.0);
    }

    let day_of = |source_id: &str| -> String {
        conn.query_row(
            "SELECT DATE(created_at) FROM source_items WHERE source_id = ?1",
            params![source_id],
            |r| r.get::<_, String>(0),
        )
        .expect("read fixture day")
    };

    assert_eq!(
        day_of("recent"),
        day_of("also-recent"),
        "two sub-hour offsets must share a calendar day"
    );
    assert_ne!(
        day_of("recent"),
        day_of("yesterday"),
        "a sub-hour offset and a -1 day offset must land on DIFFERENT calendar days, \
         whatever time the suite runs — this is the multi-day gate's whole premise"
    );
    assert_ne!(day_of("yesterday"), day_of("older"));

    // Every fixture must stay inside the 7-day candidate window and never be
    // stamped in the future.
    let (in_window, in_past): (i64, i64) = conn
        .query_row(
            "SELECT SUM(created_at >= datetime('now', '-7 days')),
                    SUM(created_at <= datetime('now'))
             FROM source_items",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("window check");
    assert_eq!(
        in_window, 4,
        "all fixtures must fall inside the 7-day window"
    );
    assert_eq!(in_past, 4, "no fixture may be stamped in the future");
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

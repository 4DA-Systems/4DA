// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! The one-pass breakdown must equal the per-dependency query, row for row.

use rusqlite::{params, Connection};

use super::super::tests::setup_test_db;
use super::*;

fn add_osv_table(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS osv_advisories (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             advisory_id TEXT NOT NULL, summary TEXT NOT NULL, details TEXT,
             package_name TEXT NOT NULL, ecosystem TEXT NOT NULL,
             affected_ranges TEXT, fixed_versions TEXT, severity_type TEXT,
             cvss_score REAL, source_url TEXT, published_at TEXT, modified_at TEXT,
             synced_at TEXT NOT NULL DEFAULT (datetime('now')),
             withdrawn_at TEXT, aliases TEXT, severity_label TEXT
         );",
    )
    .unwrap();
}

fn item(conn: &Connection, title: &str, source_type: &str, ct: Option<&str>, days: i64) -> i64 {
    conn.execute(
        "INSERT INTO source_items (title, source_type, content_type, created_at)
         VALUES (?1, ?2, ?3, datetime('now', ?4))",
        params![title, source_type, ct, format!("-{days} days")],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn link(conn: &Connection, id: i64, package: &str, match_type: &str) {
    conn.execute(
        "INSERT INTO source_item_dependencies (source_item_id, package_name, ecosystem, match_type, confidence)
         VALUES (?1, ?2, 'npm', ?3, 0.9)",
        params![id, package, match_type],
    )
    .unwrap();
}

/// A corpus built to hit every branch of the row judgement AND every corner of
/// candidate selection: registry subjects, advisories (linked, unlinked,
/// fixed, exposed), word boundaries, ASCII-only case folding, the `_`
/// wildcard, a link with no title mention, a non-structured link, an
/// out-of-window row, and a non-ASCII case fold `LIKE` does not perform.
fn fixture() -> Connection {
    let conn = setup_test_db();
    add_osv_table(&conn);
    item(
        &conn,
        "crates.io: axum v0.8.9",
        "crates_io",
        Some("release_notes"),
        1,
    );
    item(
        &conn,
        "crates.io: axum-extra v0.10.0",
        "crates_io",
        Some("release_notes"),
        2,
    );
    item(
        &conn,
        "crates.io: axum v0.8.6",
        "crates_io",
        Some("release_notes"),
        3,
    );
    item(&conn, "npm: react v19.2.8", "npm", Some("release_notes"), 1);
    item(&conn, "How Google reacted to the outage", "rss", None, 2);
    item(
        &conn,
        "React 19.3 released with compiler",
        "rss",
        Some("release_notes"),
        2,
    );
    item(
        &conn,
        "Deep dive: REACT server components",
        "hackernews",
        Some("deep_dive"),
        4,
    );
    item(
        &conn,
        "React breaking change in the router",
        "reddit",
        Some("breaking_change"),
        5,
    );
    item(
        &conn,
        "react 20 announced",
        "rss",
        Some("release_notes"),
        45,
    ); // out of window
    let hono_adv = item(
        &conn,
        "[GHSA-f23p-vx2j-j53r] hono: memo() leaks",
        "osv",
        Some("security_advisory"),
        2,
    );
    link(&conn, hono_adv, "Hono", "advisory"); // LOWER() folds the linker's case
    conn.execute(
        "UPDATE source_items SET source_id = 'GHSA-f23p-vx2j-j53r' WHERE id = ?1",
        params![hono_adv],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO osv_advisories (advisory_id, summary, package_name, ecosystem, affected_ranges)
         VALUES ('GHSA-f23p-vx2j-j53r', 'memo leak', 'hono', 'npm',
                 '[{\"type\":\"SEMVER\",\"events\":[{\"introduced\":\"0\"},{\"fixed\":\"4.12.34\"}]}]')",
        [],
    )
    .unwrap();
    item(
        &conn,
        "[CVE-2026-1] hono title-only advisory",
        "cve",
        Some("security_advisory"),
        3,
    );
    item(&conn, "Electron honors the new spec", "hackernews", None, 1);
    let heuristic = item(&conn, "Hono middleware roundup", "rss", None, 2);
    link(&conn, heuristic, "hono", "title_heuristic"); // not structured proof
    item(
        &conn,
        "serial-test 3.4.1 released",
        "rss",
        Some("release_notes"),
        2,
    );
    item(
        &conn,
        "crates.io: serial_test v3.4.1",
        "crates_io",
        Some("release_notes"),
        2,
    );
    item(
        &conn,
        "crates.io: serialXtest v1.0.0",
        "crates_io",
        Some("release_notes"),
        2,
    );
    item(&conn, "STRIPE outage postmortem", "rss", None, 1);
    item(
        &conn,
        "silverstripe 5 security fix",
        "rss",
        Some("security_advisory"),
        1,
    );
    let linked_only = item(
        &conn,
        "crates.io: tokio-macros v2.6.0",
        "crates_io",
        Some("release_notes"),
        1,
    );
    link(&conn, linked_only, "tokio", "exact_registry");
    let linked_editorial = item(
        &conn,
        "Async runtimes compared",
        "lobsters",
        Some("expert_analysis"),
        1,
    );
    link(&conn, linked_editorial, "tokio", "exact_registry");
    item(
        &conn,
        "\u{212A}it 2.0 ships",
        "rss",
        Some("release_notes"),
        1,
    ); // Kelvin sign: LIKE does not fold it
    item(&conn, "Kit 2.1 ships", "rss", Some("release_notes"), 1);
    item(
        &conn,
        "image crate 0.25 and ImageMagick news",
        "rss",
        Some("release_notes"),
        1,
    );
    conn
}

/// Every (name, ecosystem, installs) shape a report asks for.
fn requests() -> Vec<(&'static str, Option<&'static str>, Vec<String>)> {
    let v = |s: &[&str]| s.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
    vec![
        ("axum", Some("crates.io"), v(&["0.8.6"])),
        ("axum", Some("npm"), v(&[])),
        ("axum", None, v(&[])),
        ("react", Some("npm"), v(&[])),
        ("react", Some("npm"), v(&["19.2.8"])),
        ("React", None, v(&[])),
        ("hono", Some("npm"), v(&["4.11.0"])),
        ("hono", Some("npm"), v(&["4.13.3"])),
        ("hono", None, v(&[])),
        ("serial_test", Some("crates.io"), v(&["3.4.0"])),
        ("stripe", Some("npm"), v(&[])),
        ("tokio", Some("crates.io"), v(&[])),
        ("kit", None, v(&[])),
        ("image", Some("crates.io"), v(&[])),
        ("nothing-mentions-this", Some("npm"), v(&[])),
    ]
}

#[test]
fn the_shared_pass_equals_the_per_dependency_query_on_a_fixture() {
    let conn = fixture();
    let corpus = RecentCorpus::load(&conn).expect("corpus loads");
    let mut nonzero = 0;
    for (name, eco, installed) in requests() {
        let reference = count_signal_types_for_dep_conn(&conn, name, eco, &installed);
        let batched = corpus.breakdown(&conn, name, eco, &installed);
        assert_eq!(batched, reference, "{name} / {eco:?} / {installed:?}");
        if reference != DepSignalBreakdown::default() {
            nonzero += 1;
        }
    }
    // Non-vacuous: most shapes count something, in several buckets.
    assert!(nonzero >= 9, "only {nonzero} non-empty breakdowns");
    let react = corpus.breakdown(&conn, "react", Some("npm"), &[]);
    assert!(react.releases > 0 && react.analyses > 0 && react.security > 0);
    let hono = corpus.breakdown(&conn, "hono", Some("npm"), &["4.11.0".to_string()]);
    assert_eq!(hono.advisories, 1, "the linked, exposed advisory");
    assert_eq!(
        corpus
            .breakdown(&conn, "hono", Some("npm"), &["4.13.3".to_string()])
            .security,
        0,
        "the fixed install"
    );
}

/// Candidate selection reproduces SQLite's own `LIKE`, asked of SQLite
/// directly: ASCII-only folding, `_` = one character (multi-byte included),
/// `%` = any run.
#[test]
fn like_contains_agrees_with_sqlite() {
    let conn = Connection::open_in_memory().unwrap();
    let hays = [
        "serial-test 3.4",
        "serial_test",
        "SERIALXTEST",
        "serialtest",
        "serial\u{e9}test",
        "\u{212A}it 2.0",
        "Kit",
        "kIT",
        "ünïcode Ü",
        "100% done",
        "",
        "abc",
        "a_c",
        "xx%yy",
    ];
    let needles = [
        "serial_test",
        "kit",
        "ünïcode ü",
        "ünïcode",
        "%",
        "_",
        "a_c",
        "100%",
        "x%y",
        "",
        "abc",
        "c",
    ];
    for hay in hays {
        for needle in needles {
            let sqlite: bool = conn
                .query_row(
                    "SELECT ?1 LIKE '%' || ?2 || '%'",
                    params![hay, needle],
                    |r| r.get(0),
                )
                .unwrap();
            let ours =
                LikeContains::new(&needle.to_ascii_lowercase()).matches(&hay.to_ascii_lowercase());
            assert_eq!(ours, sqlite, "{hay:?} LIKE %{needle:?}%");
        }
    }
}

#[test]
fn prime_fills_the_memo_every_lookup_reads() {
    let conn = fixture();
    let installed = vec!["0.8.6".to_string()];
    let expected = count_signal_types_for_dep_conn(&conn, "axum", Some("crates.io"), &installed);
    assert!(expected.releases > 0);
    let primed = prime(
        &conn,
        &[BreakdownRequest {
            dep_name: "axum".to_string(),
            ecosystem: Some("crates.io".to_string()),
            installed: installed.clone(),
        }],
    );
    assert_eq!(primed, 1);
    // The lookup now answers from the memo — even with no corpus stand-in
    // installed (which would otherwise read as "no data").
    assert_eq!(
        super::super::count_signal_types_for_dep("axum", Some("crates.io"), &installed),
        expected
    );
    assert_eq!(
        memo_get(&memo_key("axum", Some("crates.io"), &installed)),
        Some(expected)
    );
    // An unprimed key still takes the per-dependency path (here: no stand-in,
    // so "no data", exactly as before the memo existed).
    assert_eq!(
        super::super::count_signal_types_for_dep("axum", Some("npm"), &[]),
        DepSignalBreakdown::default()
    );
}

#[test]
fn an_empty_request_list_reads_nothing() {
    let conn = Connection::open_in_memory().unwrap(); // no tables at all
    assert_eq!(prime(&conn, &[]), 0);
}

#[test]
fn an_unreadable_corpus_primes_nothing_and_lookups_fall_back() {
    let conn = Connection::open_in_memory().unwrap(); // no source_items
    let req = BreakdownRequest {
        dep_name: "never-primed".to_string(),
        ecosystem: None,
        installed: Vec::new(),
    };
    assert_eq!(prime(&conn, &[req]), 0);
    assert_eq!(memo_get(&memo_key("never-primed", None, &[])), None);
}

/// Equivalence on a REAL corpus, every dependency the coverage set knows,
/// with its own installs — plus the stale-topic shape (no ecosystem, no
/// installs). Run on a SNAPSHOT only:
///
/// ```text
/// FOURDA_DB_PATH=<snapshot> cargo test --lib \
///     live_shared_breakdown_pass_equals_per_dependency_queries -- --ignored --nocapture
/// ```
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_shared_breakdown_pass_equals_per_dependency_queries() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("open snapshot read-only");
    let deps = super::super::get_dependency_coverage(&conn).expect("coverage set");
    let mut shapes: Vec<(String, Option<String>, Vec<String>)> = Vec::new();
    for d in &deps {
        let installed = super::super::installed_versions_conn(
            &conn,
            &d.package_name,
            Some(d.ecosystem.as_str()),
            &d.projects,
        );
        shapes.push((d.package_name.clone(), Some(d.ecosystem.clone()), installed));
        shapes.push((d.package_name.clone(), None, Vec::new()));
    }
    let t = std::time::Instant::now();
    let corpus = RecentCorpus::load(&conn).expect("corpus loads");
    let load_ms = t.elapsed().as_millis();
    let t = std::time::Instant::now();
    let batched: Vec<DepSignalBreakdown> = shapes
        .iter()
        .map(|(n, e, i)| corpus.breakdown(&conn, n, e.as_deref(), i))
        .collect();
    let batch_ms = t.elapsed().as_millis();
    let t = std::time::Instant::now();
    let references: Vec<DepSignalBreakdown> = shapes
        .iter()
        .map(|(n, e, i)| count_signal_types_for_dep_conn(&conn, n, e.as_deref(), i))
        .collect();
    let per_dep_ms = t.elapsed().as_millis();
    // The window is `datetime('now', '-30 days')`: over the minutes the
    // per-dependency queries take, an item can age out of it. Re-read the
    // corpus AFTER them; a shape differs only if it matches NEITHER reading.
    let after = RecentCorpus::load(&conn).expect("corpus reloads");
    let window_moved = corpus.rows.len() != after.rows.len();
    let mut nonzero = 0usize;
    let mut drift_only = 0usize;
    let mut mismatches = Vec::new();
    for (((n, e, i), got), want) in shapes.iter().zip(&batched).zip(&references) {
        if *want != DepSignalBreakdown::default() {
            nonzero += 1;
        }
        if got == want {
            continue;
        }
        let got_after = after.breakdown(&conn, n, e.as_deref(), i);
        if got_after == *want {
            drift_only += 1;
        } else {
            mismatches.push(format!(
                "{n} / {e:?} / {i:?}: batch {got:?} / {got_after:?} vs query {want:?}"
            ));
        }
    }
    println!(
        "{} shapes over {} recent rows ({} non-empty): corpus load {load_ms} ms + shared pass \
         {batch_ms} ms vs per-dependency queries {per_dep_ms} ms; {} identical, {drift_only} \
         equal once re-read after the window moved ({} -> {} rows), {} mismatches",
        shapes.len(),
        corpus.rows.len(),
        nonzero,
        shapes.len() - drift_only - mismatches.len(),
        corpus.rows.len(),
        after.rows.len(),
        mismatches.len()
    );
    assert!(
        drift_only == 0 || window_moved,
        "a re-read cannot fix a non-drift"
    );
    for m in mismatches.iter().take(20) {
        println!("  MISMATCH {m}");
    }
    assert!(mismatches.is_empty(), "{} mismatches", mismatches.len());
}

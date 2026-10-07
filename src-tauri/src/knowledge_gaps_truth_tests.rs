// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Truthful gaps: a release names exactly the live, directly-declaring
//! projects that are behind it — the live 2026-10-07 chrono / notify shapes.

use super::super::{GapBasis, GapSeverity, KnowledgeGap, MissedItem};
use super::*;

/// Paths that do not exist on disk, so neither the liveness filesystem probe
/// nor the own-package check can read the developer's real checkouts.
const RELAY: &str = "/fx/4da/relay";
const SRC_TAURI: &str = "/fx/4da/src-tauri";
const TOOLS: &str = "/fx/tools";
const BRIDGE: &str = "/fx/tools/apps/bridge/src-tauri";
const VICTAURI: &str = "/fx/victauri";
const V_CORE: &str = "/fx/victauri/crates/victauri-core";
const V_PLUGIN: &str = "/fx/victauri/crates/victauri-plugin";
const V_CLI: &str = "/fx/victauri/crates/victauri-cli";

/// The live chrono pins, 2026-10-07: everyone on 0.4.45 but the victauri
/// copies; bridge reaches it only transitively.
fn chrono_rows() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    bool,
)> {
    vec![
        ("chrono", RELAY, "0.4.45", "rust", true, false),
        ("chrono", SRC_TAURI, "0.4.45", "rust", true, false),
        ("chrono", TOOLS, "0.4.45", "rust", true, false),
        ("chrono", BRIDGE, "0.4.45", "rust", false, false),
        ("chrono", VICTAURI, "0.4.44", "rust", true, false),
        ("chrono", V_CORE, "0.4.44", "rust", true, false),
        ("chrono", V_PLUGIN, "0.4.44", "rust", true, false),
    ]
}

fn notify_rows() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    bool,
)> {
    vec![
        ("notify", SRC_TAURI, "8.2.0", "rust", true, false),
        ("notify", BRIDGE, "6.1.1", "rust", true, false),
        ("notify", VICTAURI, "7.0.0", "rust", false, false),
        ("notify", V_CLI, "7.0.0", "rust", true, false),
    ]
}

fn item(id: i64, title: &str, source_type: &str) -> MissedItem {
    MissedItem {
        item_id: id,
        title: title.to_string(),
        url: None,
        source_type: source_type.to_string(),
        created_at: "2026-09-27 13:03:00".to_string(),
    }
}

fn empty_conn() -> rusqlite::Connection {
    rusqlite::Connection::open_in_memory().unwrap()
}

fn rust() -> Vec<String> {
    vec!["rust".to_string()]
}

fn behind_paths(t: &ReleaseTriage) -> Vec<&str> {
    t.behind.iter().map(|b| b.path.as_str()).collect()
}

#[test]
fn mixed_pins_name_only_the_projects_behind() {
    let book = PinBook::from_rows(&chrono_rows(), ProjectLiveness::default());
    let t = triage_releases(
        &empty_conn(),
        &book,
        "chrono",
        &rust(),
        vec![item(123_549, "crates.io: chrono v0.4.45", "crates_io")],
    );
    assert_eq!(behind_paths(&t), vec![VICTAURI, V_CORE, V_PLUGIN]);
    assert!(t.behind.iter().all(|b| b.installed.to_string() == "0.4.44"));
    assert_eq!(
        t.latest.as_ref().map(ToString::to_string).as_deref(),
        Some("0.4.45")
    );
    assert_eq!(t.worst, Some(ReleaseClass::Patch));
    assert_eq!(t.kept.len(), 1);
}

#[test]
fn a_breaking_release_names_the_direct_laggards_not_the_current_project() {
    let book = PinBook::from_rows(&notify_rows(), ProjectLiveness::default());
    let t = triage_releases(
        &empty_conn(),
        &book,
        "notify",
        &rust(),
        vec![item(1, "crates.io: notify v8.2.0", "crates_io")],
    );
    assert_eq!(
        behind_paths(&t),
        vec![BRIDGE, V_CLI],
        "src-tauri already runs 8.2.0; the transitive victauri root is upgraded by its parent"
    );
    assert_eq!(t.worst, Some(ReleaseClass::Breaking));
}

#[test]
fn a_release_every_project_runs_is_no_gap() {
    let rows: Vec<_> = chrono_rows()
        .into_iter()
        .map(|(n, p, _, e, d, dev)| (n, p, "0.4.45", e, d, dev))
        .collect();
    let book = PinBook::from_rows(&rows, ProjectLiveness::default());
    let t = triage_releases(
        &empty_conn(),
        &book,
        "chrono",
        &rust(),
        vec![
            item(1, "crates.io: chrono v0.4.45", "crates_io"),
            item(2, "crates.io: chrono v0.4.44", "crates_io"),
            item(3, "Announcing chrono 0.4.45", "rss"),
        ],
    );
    assert!(t.kept.is_empty(), "nothing is news: {:?}", t.kept);
    assert!(t.behind.is_empty());
}

#[test]
fn a_cross_ecosystem_namesake_is_ignored() {
    // Only Rust projects carry `chrono`; an npm package of the same name is
    // somebody else's.
    let book = PinBook::from_rows(&chrono_rows(), ProjectLiveness::default());
    let t = triage_releases(
        &empty_conn(),
        &book,
        "chrono",
        &rust(),
        vec![item(1, "npm: chrono v9.0.0", "npm_registry")],
    );
    assert!(
        t.kept.is_empty(),
        "an npm namesake is neither a gap nor a citation"
    );
    assert!(t.behind.is_empty());
}

#[test]
fn a_dormant_project_is_not_behind_anything() {
    let liveness = ProjectLiveness::from_entries(&[
        (VICTAURI, 400),
        (V_CORE, 400),
        (V_PLUGIN, 400),
        (RELAY, 1),
        (SRC_TAURI, 1),
        (TOOLS, 1),
    ]);
    let book = PinBook::from_rows(&chrono_rows(), liveness);
    let t = triage_releases(
        &empty_conn(),
        &book,
        "chrono",
        &rust(),
        vec![item(1, "crates.io: chrono v0.4.45", "crates_io")],
    );
    assert!(
        t.behind.is_empty(),
        "only dormant copies lag: {:?}",
        t.behind
    );
    assert!(t.kept.is_empty());
}

#[test]
fn a_scratch_project_is_not_behind_anything() {
    let liveness = ProjectLiveness::default().with_scratch(&[VICTAURI, V_CORE, V_PLUGIN]);
    let book = PinBook::from_rows(&chrono_rows(), liveness);
    let t = triage_releases(
        &empty_conn(),
        &book,
        "chrono",
        &rust(),
        vec![item(1, "crates.io: chrono v0.4.45", "crates_io")],
    );
    assert!(t.behind.is_empty());
}

#[test]
fn editorial_versions_are_graded_and_discussion_stays_a_citation() {
    let rows = vec![("axum", SRC_TAURI, "0.8.1", "rust", true, false)];
    let book = PinBook::from_rows(&rows, ProjectLiveness::default());
    let t = triage_releases(
        &empty_conn(),
        &book,
        "axum",
        &rust(),
        vec![
            item(1, "Announcing axum 0.8.0", "rss"),
            item(2, "Critical vulnerability found in axum", "hackernews"),
        ],
    );
    let kept: Vec<i64> = t.kept.iter().map(|m| m.item_id).collect();
    assert_eq!(
        kept,
        vec![2],
        "an already-installed announcement drops; the story stays"
    );
    assert!(t.behind.is_empty());

    let behind_book = PinBook::from_rows(
        &[("axum", SRC_TAURI, "0.7.9", "rust", true, false)],
        ProjectLiveness::default(),
    );
    let t = triage_releases(
        &empty_conn(),
        &behind_book,
        "axum",
        &rust(),
        vec![
            item(1, "Announcing axum 0.8.0", "rss"),
            // Live 2026-10-07: parsed as "uuid 4.0.0".
            item(2, "UUID v4 vs. UUID v7 vs. ULID in 2026", "devto"),
        ],
    );
    assert_eq!(
        t.kept.len(),
        2,
        "an editorial announcement stays a citation"
    );
    assert!(
        t.behind.is_empty(),
        "editorial text never proves a project behind — only a registry row does"
    );

    let uuid_book = PinBook::from_rows(
        &[("uuid", SRC_TAURI, "1.26.1", "rust", true, false)],
        ProjectLiveness::default(),
    );
    let t = triage_releases(
        &empty_conn(),
        &uuid_book,
        "uuid",
        &rust(),
        vec![item(2, "UUID v4 vs. UUID v7 vs. ULID in 2026", "devto")],
    );
    assert!(t.behind.is_empty(), "\"UUID v4\" is not uuid 4.0.0");
    assert!(t.latest.is_none());
}

#[test]
fn version_labels_span_the_attributed_copies() {
    assert_eq!(version_label(["0.4.44"]).as_deref(), Some("0.4.44"));
    assert_eq!(
        version_label(["7.0.0", "6.1.1"]).as_deref(),
        Some("6.1.1\u{2013}7.0.0")
    );
    assert_eq!(version_label(["not-a-version"]), None);
}

fn gap(basis: GapBasis) -> KnowledgeGap {
    KnowledgeGap {
        dependency: "chrono".to_string(),
        version: Some("0.4.44".to_string()),
        project_path: format!("{VICTAURI} (+2 more)"),
        projects: vec![VICTAURI.into(), V_CORE.into(), V_PLUGIN.into()],
        basis,
        latest_release: Some("0.4.45".to_string()),
        missed_items: vec![item(1, "crates.io: chrono v0.4.45", "crates_io")],
        gap_severity: GapSeverity::Low,
        days_since_last_engagement: 999,
    }
}

#[test]
fn confidence_comes_from_the_evidence_never_a_constant() {
    use crate::evidence::ConfidenceProvenance as P;
    let items: Vec<_> = [GapBasis::Advisory, GapBasis::Release, GapBasis::Editorial]
        .into_iter()
        .map(|b| gap(b).to_evidence_item())
        .collect();
    let got: Vec<(f32, P)> = items
        .iter()
        .map(|i| (i.confidence.value, i.confidence.provenance))
        .collect();
    assert_eq!(
        got,
        vec![
            (0.95, P::OsvVerified),
            (0.9, P::Checklist),
            (0.5, P::Heuristic)
        ]
    );
    for item in &items {
        assert!(
            crate::evidence::validate_item(item).is_ok(),
            "{:?} must pass the runtime schema validator",
            item.confidence
        );
    }
}

#[test]
fn a_release_gap_reads_n_projects_behind_without_unread_framing() {
    let item = gap(GapBasis::Release).to_evidence_item();
    assert_eq!(item.title, "3 projects behind chrono 0.4.45");
    assert!(
        item.explanation
            .starts_with("3 projects behind chrono 0.4.45: fx/victauri (+2 more) on 0.4.44"),
        "{}",
        item.explanation
    );
    for banned in ["never reviewed", "nread", "last reviewed"] {
        assert!(!item.explanation.contains(banned), "{}", item.explanation);
        assert!(!item.evidence[0].relevance_note.contains(banned));
    }
    assert_eq!(
        item.affected_projects,
        vec![
            VICTAURI.to_string(),
            V_CORE.to_string(),
            V_PLUGIN.to_string()
        ]
    );
}

// ---------------------------------------------------------------------------
// End to end on a fully migrated database: the whole detection pass.
// ---------------------------------------------------------------------------

fn seed_e2e(conn: &rusqlite::Connection, rows: &[(&str, &str, &str, &str, bool, bool)]) {
    for (name, path, version, eco, direct, dev) in rows {
        conn.execute(
            "INSERT OR IGNORE INTO project_dependencies
                 (project_path, manifest_type, package_name, version, is_dev, is_direct,
                  language, last_scanned)
             VALUES (?1, 'cargo', ?2, NULL, ?3, ?4, ?5, datetime('now'))",
            rusqlite::params![path, name, *dev as i32, *direct as i32, eco],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem,
                                            is_dev, is_direct)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![path, name, version, eco, *dev as i32, *direct as i32],
        )
        .unwrap();
    }
}

fn add_release(conn: &rusqlite::Connection, id: i64, title: &str) {
    conn.execute(
        "INSERT INTO source_items (id, source_type, source_id, title, url, content,
                                   content_hash, embedding, created_at)
         VALUES (?1, 'crates_io', ?2, ?3, NULL, '', ?2, X'', datetime('now', '-1 day'))",
        rusqlite::params![id, format!("crate-{id}"), title],
    )
    .unwrap();
}

#[test]
fn detection_end_to_end_attributes_the_chrono_gap_to_the_laggards_only() {
    let db = crate::test_utils::test_db();
    let conn = db.conn.lock();
    seed_e2e(&conn, &chrono_rows());
    seed_e2e(&conn, &notify_rows());
    add_release(&conn, 9001, "crates.io: chrono v0.4.45");
    add_release(&conn, 9002, "crates.io: notify v8.2.0");

    let gaps = super::super::detect_knowledge_gaps(&conn).unwrap();
    let chrono = gaps
        .iter()
        .find(|g| g.dependency == "chrono")
        .expect("chrono gap");
    assert_eq!(chrono.basis, GapBasis::Release);
    assert_eq!(chrono.projects, vec![VICTAURI, V_CORE, V_PLUGIN]);
    assert_eq!(chrono.version.as_deref(), Some("0.4.44"));
    assert_eq!(chrono.latest_release.as_deref(), Some("0.4.45"));

    let notify = gaps
        .iter()
        .find(|g| g.dependency == "notify")
        .expect("notify gap");
    assert_eq!(notify.projects, vec![BRIDGE, V_CLI]);
    assert_eq!(notify.version.as_deref(), Some("6.1.1\u{2013}7.0.0"));
    assert!(!notify.projects.iter().any(|p| p == SRC_TAURI));

    // A click on the release is engagement: the row is read, the gap goes.
    // (ACE owns the canonical `interactions` schema; mirror it here.)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS interactions (
             id INTEGER PRIMARY KEY AUTOINCREMENT, source_item_id INTEGER, item_id INTEGER,
             action TEXT, action_type TEXT, action_data TEXT, item_topics TEXT,
             item_source TEXT, signal_strength REAL DEFAULT 0.5,
             timestamp TEXT DEFAULT (datetime('now')));",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO interactions (item_id, action_type) VALUES (9001, 'click')",
        [],
    )
    .unwrap();
    let gaps = super::super::detect_knowledge_gaps(&conn).unwrap();
    assert!(gaps.iter().all(|g| g.dependency != "chrono"));
}

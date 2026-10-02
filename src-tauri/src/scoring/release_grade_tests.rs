// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use super::*;

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

fn pin(path: &str, version: &str, direct: bool) -> (String, String, bool, bool) {
    (path.to_string(), version.to_string(), direct, false)
}

#[test]
fn gap_follows_cargo_semver_compatibility() {
    assert_eq!(gap(&v("5.13.4"), &v("7.1.0")), Some(ReleaseGap::Breaking));
    assert_eq!(
        gap(&v("0.10.9"), &v("0.11.0")),
        Some(ReleaseGap::Breaking),
        "0.x minor is breaking"
    );
    assert_eq!(
        gap(&v("0.1.44"), &v("0.1.45")),
        Some(ReleaseGap::Patch),
        "0.x patch is a patch"
    );
    assert_eq!(gap(&v("1.52.3"), &v("1.53.1")), Some(ReleaseGap::Minor));
    assert_eq!(gap(&v("2.0.18"), &v("2.0.21")), Some(ReleaseGap::Patch));
    assert_eq!(gap(&v("0.11.0"), &v("0.11.0")), None, "already on it");
    assert_eq!(
        gap(&v("1.2.0"), &v("1.1.9")),
        None,
        "a backport is not an upgrade"
    );
    assert_eq!(
        gap(&v("2.11.6"), &v("3.0.0-alpha.2")),
        Some(ReleaseGap::Breaking),
        "the gap itself ignores prerelease-ness; the class decides"
    );
}

/// The live fastembed case: one direct project two majors behind.
#[test]
fn fastembed_major_is_a_breaking_upgrade_naming_the_project() {
    let g = ReleaseGrade::from_pins(
        "fastembed",
        v("7.1.0"),
        vec![pin("d:/4da/src-tauri", "5.13.4", true)],
        &[],
    );
    assert_eq!(g.class(), Some(ReleaseClass::Breaking));
    assert!(matches!(
        g.class(),
        Some(ReleaseClass::Breaking | ReleaseClass::Yanked)
    ));
    assert_eq!(g.headline_installed().as_deref(), Some("5.13.4"));
    assert_eq!(
        g.concerned_label().as_deref(),
        Some("4da/src-tauri on 5.13.4")
    );
    assert_eq!(
        g.necessity_reason().as_deref(),
        Some("Breaking upgrade: fastembed 7.1.0 for 4da/src-tauri on 5.13.4")
    );
}

/// The live sha2 case: 4DA is current, two sibling projects are behind.
#[test]
fn sha2_names_the_projects_that_are_behind_not_the_current_one() {
    let g = ReleaseGrade::from_pins(
        "sha2",
        v("0.11.0"),
        vec![
            pin("d:/4da/src-tauri", "0.11.0", true),
            pin("d:/work/tools", "0.10.9", true),
            pin("d:/work/web", "0.10.9", true),
        ],
        &[],
    );
    assert_eq!(g.class(), Some(ReleaseClass::Breaking));
    assert_eq!(
        g.headline_installed().as_deref(),
        Some("0.10.9"),
        "not the 4DA copy"
    );
    let label = g.concerned_label().unwrap();
    assert!(
        label.starts_with("work/tools (+1 more) on 0.10.9"),
        "{label}"
    );
    let (display, evidence) = g.chain_text().unwrap();
    assert_eq!(
        display,
        "Breaking upgrade: sha2 0.10.9 \u{2192} 0.11.0 (work/tools (+1 more))"
    );
    assert!(
        evidence.contains("already current: 4da/src-tauri"),
        "{evidence}"
    );
}

#[test]
fn every_project_current_has_no_class() {
    let g = ReleaseGrade::from_pins(
        "uuid",
        v("1.26.1"),
        vec![
            pin("d:/4da/src-tauri", "1.26.1", true),
            pin("d:/4da/relay", "1.26.2", true),
        ],
        &[],
    );
    assert!(g.all_installed());
    assert_eq!(g.class(), None);
    assert_eq!(g.necessity_reason(), None);
}

#[test]
fn only_transitive_copies_behind_is_not_actionable() {
    let g = ReleaseGrade::from_pins(
        "sha2",
        v("0.11.0"),
        vec![
            pin("d:/4da/src-tauri", "0.11.0", true),
            pin("d:/work/tools", "0.10.9", false),
        ],
        &[],
    );
    assert_eq!(
        g.class(),
        Some(ReleaseClass::Patch),
        "a transitive copy is upgraded by its parent, not by the user"
    );
    assert!(!matches!(
        g.class(),
        Some(ReleaseClass::Breaking | ReleaseClass::Yanked)
    ));
}

#[test]
fn largest_direct_gap_wins_and_only_its_projects_are_named() {
    let g = ReleaseGrade::from_pins(
        "tokio",
        v("1.53.1"),
        vec![
            pin("d:/4da/relay", "1.53.0", true),
            pin("d:/4da/src-tauri", "1.52.3", true),
        ],
        &[],
    );
    assert_eq!(g.class(), Some(ReleaseClass::Minor));
    assert_eq!(
        g.concerned_label().as_deref(),
        Some("4da/src-tauri on 1.52.3")
    );
}

#[test]
fn patch_only_is_patch() {
    let g = ReleaseGrade::from_pins(
        "thiserror",
        v("2.0.21"),
        vec![pin("d:/4da/src-tauri", "2.0.18", true)],
        &[],
    );
    assert_eq!(g.class(), Some(ReleaseClass::Patch));
}

#[test]
fn a_prerelease_is_awareness_even_across_a_major() {
    let g = ReleaseGrade::from_pins(
        "tauri",
        v("3.0.0-alpha.2"),
        vec![pin("d:/4da/src-tauri", "2.11.6", true)],
        &[],
    );
    assert_eq!(g.class(), Some(ReleaseClass::Prerelease));
    assert!(!matches!(
        g.class(),
        Some(ReleaseClass::Breaking | ReleaseClass::Yanked)
    ));
    assert_eq!(
        g.necessity_reason().as_deref(),
        Some("Pre-release tauri 3.0.0-alpha.2 (4da/src-tauri on 2.11.6 stays on stable)")
    );
}

#[test]
fn a_yanked_pin_is_actionable_even_when_current() {
    let g = ReleaseGrade::from_pins(
        "rusqlite",
        v("0.26.1"),
        vec![pin("d:/4da/src-tauri", "0.26.1", true)],
        &["0.26.1".to_string(), "0.26.0".to_string()],
    );
    assert!(g.all_installed(), "the version is current");
    assert_eq!(
        g.class(),
        Some(ReleaseClass::Yanked),
        "but it was withdrawn"
    );
    assert!(matches!(
        g.class(),
        Some(ReleaseClass::Breaking | ReleaseClass::Yanked)
    ));
}

#[test]
fn yanked_list_parses_from_the_crates_io_body() {
    let body = "Ergonomic wrapper for SQLite\nDownloads: 101385377\nYanked versions: 0.26.1, 0.26.0, 0.25.3\n";
    assert_eq!(yanked_versions(body), vec!["0.26.1", "0.26.0", "0.25.3"]);
    assert!(yanked_versions("no list here").is_empty());
}

#[test]
fn a_project_listed_twice_keeps_its_direct_copy() {
    let g = ReleaseGrade::from_pins(
        "sha2",
        v("0.11.0"),
        vec![
            pin("D:\\4DA\\relay", "0.10.9", false),
            pin("d:/4da/relay", "0.11.0", true),
        ],
        &[],
    );
    assert_eq!(g.pins.len(), 1);
    assert!(g.pins[0].is_direct);
    assert_eq!(g.class(), None, "the direct copy is current");
}

#[test]
fn unparseable_versions_are_ignored_not_guessed() {
    let g = ReleaseGrade::from_pins("x", v("1.0.0"), vec![pin("d:/p", "workspace", true)], &[]);
    assert!(g.pins.is_empty());
    assert_eq!(g.class(), None);
    assert!(
        !g.all_installed(),
        "no pins is 'cannot tell', never 'installed'"
    );
}

#[test]
fn loader_reads_every_project_in_the_registry_language_only() {
    let db = crate::test_utils::test_db();
    db.store_dependency(
        "/proj/app",
        "fastembed",
        Some("5.13.4"),
        "rust",
        false,
        None,
    )
    .unwrap();
    db.store_dependency(
        "/proj/web",
        "fastembed",
        Some("1.0.0"),
        "javascript",
        false,
        None,
    )
    .unwrap();
    let g = grade_registry_release(
        &db,
        "crates_io",
        "crates.io: fastembed v7.1.0",
        "Library for generating vector embeddings.",
    )
    .expect("the rust pin grades the crates.io row");
    assert_eq!(
        g.pins.len(),
        1,
        "an npm package of the same name is another ecosystem"
    );
    assert_eq!(g.class(), Some(ReleaseClass::Breaking));
    assert!(
        grade_registry_release(&db, "hackernews", "fastembed 7.1.0 is out", "").is_none(),
        "editorial rows are not graded"
    );
    assert!(
        grade_registry_release(&db, "crates_io", "crates.io: unknowncrate v1.0.0", "").is_none(),
        "no pins, no grade"
    );
}

#[test]
fn loader_reads_the_transitive_flag() {
    let db = crate::test_utils::test_db();
    db.store_dependency("/proj/app", "sha2", Some("0.10.9"), "rust", false, None)
        .unwrap();
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE user_dependencies SET is_direct = 0 WHERE package_name = 'sha2'",
            [],
        )
        .unwrap();
    }
    let g = grade_registry_release(&db, "crates_io", "crates.io: sha2 v0.11.0", "").unwrap();
    assert!(!g.pins[0].is_direct);
    assert_eq!(g.class(), Some(ReleaseClass::Patch));
}

/// Walk the live rows through the grade with the REAL ownership check
/// (lockfiles on this machine). Opens the database read-only — never through
/// `Database`, whose open runs migrations.
/// `FOURDA_VERIFY_DB=D:/4DA/data/4da.db cargo test --lib live_registry_release_walkthrough -- --ignored --nocapture`
#[test]
#[ignore = "requires FOURDA_VERIFY_DB pointing at a real database"]
fn live_registry_release_walkthrough() {
    let Ok(path) = std::env::var("FOURDA_VERIFY_DB") else {
        return;
    };
    let conn = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("open read-only");
    let titles: Vec<(i64, String, String, String)> = conn
        .prepare(
            "SELECT id, title, source_type, COALESCE(content, '') FROM source_items
             WHERE content_type = 'release_notes' AND feed_relevant = 1
               AND source_type IN ('crates_io', 'npm_registry')
             ORDER BY relevance_score DESC",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .flatten()
        .collect();
    let user_excluded = crate::project_inclusion::user_excluded_paths();
    for (id, title, source_type, content) in titles {
        let Some((subject, Some(raw))) = crate::dep_linker::registry_title_subject(&title) else {
            continue;
        };
        let Some(announced) = lenient_semver(&raw, None) else {
            continue;
        };
        let Some(lang) = dependencies::registry_manifest_language(&source_type) else {
            continue;
        };
        let pins: Vec<(String, String, bool, bool)> = conn
            .prepare(
                "SELECT project_path, version, ecosystem, is_direct, is_dev FROM user_dependencies
                 WHERE LOWER(REPLACE(package_name, '_', '-')) = LOWER(REPLACE(?1, '_', '-'))
                   AND version IS NOT NULL AND version <> ''",
            )
            .unwrap()
            .query_map([&subject], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)? != 0,
                    r.get::<_, i64>(4)? != 0,
                ))
            })
            .unwrap()
            .flatten()
            .filter(|(p, _, eco, _, _)| {
                dependencies::ecosystem_congruent(lang, eco)
                    && !crate::project_inclusion::is_excluded_from_intelligence(p, &user_excluded)
            })
            .map(|(p, v, _, d, dev)| (p, v, d, dev))
            .collect();
        let yanked = yanked_versions(&content);
        let grade = grade_pins(&subject, announced, pins, &yanked, |p| {
            super::super::release_ownership::is_own_package(p, &subject, lang)
        });
        let verdict = match &grade {
            None => "UNGRADED".to_string(),
            Some(g) if g.not_news() => "HELD (not news)".to_string(),
            Some(g) => format!("{:?}", g.class()),
        };
        let display = grade
            .as_ref()
            .and_then(ReleaseGrade::chain_text)
            .map(|(d, _)| d)
            .unwrap_or_default();
        println!("LIVE {id} | {title} | {verdict} | {display}");
    }
}

// ── Live 2026-10-02 (adversarial audit of the Signal feed) ───────────────
// Pins below are the live `user_dependencies` rows for each package.

fn dev_pin(path: &str, version: &str, direct: bool) -> (String, String, bool, bool) {
    (path.to_string(), version.to_string(), direct, true)
}

/// The victauri workspace (root + members) builds the crate, and the
/// gauntlet takes victauri-test by path. Ownership as the lockfiles report
/// it on the founder machine.
fn victauri_owns(path: &str) -> bool {
    path.starts_with("d:/runyourempire/victauri") || path == "d:/4da/victauri-gauntlet"
}

/// `crates.io: victauri-core v0.9.0` read "Breaking upgrade of your
/// dependency victauri-core" — graded against the victauri workspace's own
/// members on 0.8.8. The one real pin is a sibling app's bridge on 0.8.4; 4DA (the
/// operator's other project) already runs 0.9.0.
#[test]
fn victauri_core_is_graded_only_against_the_projects_that_install_it() {
    let g = grade_pins(
        "victauri-core",
        v("0.9.0"),
        vec![
            pin("d:/4da/src-tauri", "0.9.0", false),
            pin("d:/work/agent-hub/apps/bridge/src-tauri", "0.8.4", true),
            pin("d:/runyourempire/victauri", "0.8.8", false),
            pin(
                "d:/runyourempire/victauri/crates/victauri-cli",
                "0.8.8",
                true,
            ),
            dev_pin(
                "d:/runyourempire/victauri/crates/victauri-macros",
                "0.8.8",
                true,
            ),
            pin(
                "d:/runyourempire/victauri/crates/victauri-plugin",
                "0.8.8",
                true,
            ),
        ],
        &[],
        victauri_owns,
    )
    .unwrap();
    assert_eq!(g.own_projects.len(), 4, "{:?}", g.own_projects);
    assert_eq!(g.class(), Some(ReleaseClass::Breaking));
    assert!(!g.not_news());
    assert_eq!(
        g.concerned_label().as_deref(),
        Some("bridge/src-tauri on 0.8.4"),
        "the lagging sibling is named, not 4DA and not the workspace"
    );
    let (display, evidence) = g.chain_text().unwrap();
    assert_eq!(
        display,
        "Breaking upgrade: victauri-core 0.8.4 \u{2192} 0.9.0 (bridge/src-tauri)"
    );
    assert!(
        evidence.contains("already current: 4da/src-tauri"),
        "{evidence}"
    );
    assert!(evidence.contains("built in your own"), "{evidence}");
}

/// `victauri-macros v0.9.0`: once the workspace is set aside, only a
/// transitive copy is behind (a sibling app's bridge reaches it through
/// victauri-plugin) — a patch-class row, out of the feed.
#[test]
fn victauri_macros_has_no_project_that_can_act_on_it() {
    let g = grade_pins(
        "victauri-macros",
        v("0.9.0"),
        vec![
            pin("d:/4da/src-tauri", "0.9.0", false),
            pin("d:/work/agent-hub/apps/bridge/src-tauri", "0.8.4", false),
            pin("d:/runyourempire/victauri", "0.8.8", false),
            pin(
                "d:/runyourempire/victauri/crates/victauri-plugin",
                "0.8.8",
                true,
            ),
        ],
        &[],
        victauri_owns,
    )
    .unwrap();
    assert_eq!(g.class(), Some(ReleaseClass::Patch));
}

/// `victauri-test v0.9.0`: 4DA (direct) runs 0.9.0; the gauntlet takes it
/// by path and the workspace builds it. Nobody who installs it is behind.
#[test]
fn victauri_test_is_not_news() {
    let g = grade_pins(
        "victauri-test",
        v("0.9.0"),
        vec![
            pin("d:/4da/src-tauri", "0.9.0", true),
            pin("d:/4da/victauri-gauntlet", "0.8.8", false),
            pin("d:/runyourempire/victauri", "0.8.8", false),
            pin(
                "d:/runyourempire/victauri/crates/victauri-cli",
                "0.8.8",
                true,
            ),
        ],
        &[],
        victauri_owns,
    )
    .unwrap();
    assert_eq!(g.class(), None);
    assert!(g.not_news(), "every installing project already runs it");
}

/// A package only the user's own workspace carries is their own
/// publication announced back to them.
#[test]
fn a_package_only_its_own_workspace_carries_is_not_news() {
    let g = grade_pins(
        "victauri-watchdog",
        v("0.9.0"),
        vec![pin("d:/runyourempire/victauri", "0.8.8", false)],
        &[],
        victauri_owns,
    )
    .unwrap();
    assert!(g.pins.is_empty());
    assert_eq!(g.class(), None);
    assert!(g.not_news());
    let (display, evidence) = g.chain_text().unwrap();
    assert_eq!(display, "Your own package victauri-watchdog");
    assert!(
        evidence.contains("built in runyourempire/victauri"),
        "{evidence}"
    );
    assert!(
        grade_pins("x", v("1.0.0"), Vec::new(), &[], |_| true).is_none(),
        "no project at all is 'cannot tell', not 'own'"
    );
}

/// `crates.io: tokio v1.53.1`: 4DA (src-tauri, relay) runs it since #780,
/// but the gauntlet, a sibling workspace and victauri declare tokio DIRECTLY on 1.52.x.
/// By the module's doctrine a new minor is worth knowing for a direct
/// declarer — the row stays, and now says for whom.
#[test]
fn tokio_minor_names_the_lagging_direct_declarers() {
    let g = grade_pins(
        "tokio",
        v("1.53.1"),
        vec![
            pin("d:/4da/relay", "1.53.1", true),
            pin("d:/4da/src-tauri", "1.53.1", true),
            pin("d:/4da/victauri-gauntlet", "1.52.1", true),
            pin("d:/work/agent-hub", "1.52.3", true),
            pin("d:/work/agent-hub/apps/bridge/src-tauri", "1.52.3", false),
            pin("d:/work/agent-hub/crates/hubd", "1.52.3", true),
            pin("d:/runyourempire/victauri", "1.52.1", true),
        ],
        &[],
        |_| false,
    )
    .unwrap();
    assert_eq!(g.class(), Some(ReleaseClass::Minor));
    assert!(!g.not_news());
    let (display, evidence) = g.chain_text().unwrap();
    assert_eq!(
        display,
        "New minor: tokio 1.52.1\u{2013}1.52.3 \u{2192} 1.53.1 (4da/victauri-gauntlet (+3 more))"
    );
    assert!(
        evidence.contains("already current: 4da/relay (+1 more)"),
        "{evidence}"
    );
}

/// `npm: react v19.3.0`: 4DA runs it; navcal (19.2.0) and a sibling app's bridge UI
/// (19.2.7) declare react directly — a new minor for them, not for 4DA.
#[test]
fn react_minor_names_navcal_and_the_bridge_not_4da() {
    let g = grade_pins(
        "react",
        v("19.3.0"),
        vec![
            pin("d:/4da", "19.3.0", true),
            pin("d:/runyourempire/navcal", "19.2.0", true),
            pin("d:/work/agent-hub/apps/bridge", "19.2.7", true),
        ],
        &[],
        |_| false,
    )
    .unwrap();
    assert_eq!(g.class(), Some(ReleaseClass::Minor));
    assert_eq!(
        g.concerned_label().as_deref(),
        Some("runyourempire/navcal (+1 more) on 19.2.0\u{2013}19.2.7")
    );
}

/// The rule the audit asked for, stated directly: when every DIRECT
/// declarer runs the release or newer, the row is at most Patch (only
/// transitive copies lag) or not news at all.
#[test]
fn every_direct_declarer_current_is_never_above_patch() {
    let behind_transitively = grade_pins(
        "uuid",
        v("1.26.1"),
        vec![
            pin("d:/4da/src-tauri", "1.26.1", true),
            pin("d:/work/agent-hub/apps/bridge/src-tauri", "1.23.3", false),
        ],
        &[],
        |_| false,
    )
    .unwrap();
    assert_eq!(behind_transitively.class(), Some(ReleaseClass::Patch));
    let all_current = grade_pins(
        "uuid",
        v("1.26.1"),
        vec![
            pin("d:/4da/src-tauri", "1.26.1", true),
            pin("d:/4da/relay", "1.27.0", true),
        ],
        &[],
        |_| false,
    )
    .unwrap();
    assert!(all_current.not_news());
}

/// A yanked pin is news even when every project is on the announced
/// version — `not_news` never hides it.
#[test]
fn a_yanked_current_pin_is_still_news() {
    let g = grade_pins(
        "rusqlite",
        v("0.26.1"),
        vec![pin("d:/4da/src-tauri", "0.26.1", true)],
        &["0.26.1".to_string()],
        |_| false,
    )
    .unwrap();
    assert!(!g.not_news());
    assert_eq!(g.class(), Some(ReleaseClass::Yanked));
    let (display, _) = g.chain_text().unwrap();
    assert_eq!(
        display,
        "Your pinned rusqlite 0.26.1 was yanked (4da/src-tauri)"
    );
}

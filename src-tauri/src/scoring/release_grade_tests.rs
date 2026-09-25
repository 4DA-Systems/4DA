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
    assert_eq!(display, "Breaking upgrade of your dependency sha2");
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

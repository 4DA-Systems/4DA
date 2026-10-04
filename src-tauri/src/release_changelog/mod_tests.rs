// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The lane end to end without the network: archive bytes -> record -> card,
//! the grade gate on which rows qualify, and the backlog's dedup / cache skip.

use std::io::Write;

use super::*;

/// A gzip tar holding `files` (path, body), ustar headers only.
fn archive(files: &[(&str, &str)]) -> Vec<u8> {
    let mut tar = Vec::new();
    for (path, body) in files {
        let mut h = vec![0u8; 512];
        h[..path.len()].copy_from_slice(path.as_bytes());
        h[100..108].copy_from_slice(b"0000644\0");
        h[124..136].copy_from_slice(format!("{:011o}\0", body.len()).as_bytes());
        h[156] = b'0';
        h[257..263].copy_from_slice(b"ustar\0");
        h[148..156].fill(b' ');
        let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
        h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        tar.extend(h);
        let mut data = body.as_bytes().to_vec();
        data.resize(body.len().div_ceil(512) * 512, 0);
        tar.extend(data);
    }
    tar.extend(vec![0u8; 1024]);
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&tar).unwrap();
    enc.finish().unwrap()
}

const RTEN_CHANGELOG: &str = "# Changelog

## [0.27.0] - 2026-09-20

### Breaking changes

- `Model::run` now returns a `Result<Vec<Value>>` instead of `Vec<Value>`
- Removed the deprecated `ModelOptions::with_threads` method

### Added

- Support for the `GatherND` operator

### Fixed

- Fixed a panic when loading empty tensors

## [0.26.0] - 2026-08-01

### Added

- New `Tensor::slice_with` API

### Fixed

- Fixed incorrect output of `Softmax` on NaN input
- Correct shape inference for `Concat`

## [0.25.0] - 2026-07-01

### Changed

- Bump MSRV to 1.85

## [0.24.0] - 2026-06-01

- Initial changes
";

fn target(eco: Ecosystem, package: &str, from: &str, to: &str) -> ReleaseTarget {
    ReleaseTarget {
        eco,
        package: package.into(),
        from: from.into(),
        to: to.into(),
    }
}

#[test]
fn an_archive_with_a_changelog_becomes_a_found_record() {
    let gz = archive(&[
        ("rten-0.27.0/Cargo.toml", "[package]"),
        ("rten-0.27.0/CHANGELOG.md", RTEN_CHANGELOG),
    ]);
    let rec = record_from_archive(&gz);
    assert_eq!(rec.status, RecordStatus::Found);
    assert_eq!(rec.file.as_deref(), Some("rten-0.27.0/CHANGELOG.md"));
    assert_eq!(parse_changelog(rec.text.as_deref().unwrap()).len(), 4);
}

#[test]
fn the_card_counts_the_range_and_lists_breaking_entries_first_found() {
    let rec = record_from_archive(&archive(&[("rten-0.27.0/CHANGELOG.md", RTEN_CHANGELOG)]));
    let t = target(Ecosystem::Crates, "rten", "0.24.0", "0.27.0");
    let card = summarize(&t, &rec);
    assert_eq!(card.status, ReleaseChangesStatus::Found);
    assert_eq!(card.versions, ["0.27.0", "0.26.0", "0.25.0"]);
    assert!(card.covers_range);
    // 0.27: 2 breaking + 1 feature + 1 fix; 0.26: 1 feature + 2 fixes; 0.25: MSRV bump.
    assert_eq!((card.breaking, card.features, card.fixes), (3, 2, 3));
    assert_eq!(card.top_breaking.len(), 3);
    assert_eq!(card.top_breaking[0].version, "0.27.0");
    assert!(card.top_breaking[0]
        .text
        .starts_with("`Model::run` now returns"));
    assert_eq!(card.top_breaking[2].text, "Bump MSRV to 1.85");
    assert_eq!(
        card.source_url.as_deref(),
        Some("https://docs.rs/crate/rten/0.27.0/source/CHANGELOG.md")
    );
}

#[test]
fn a_changelog_that_stops_short_reports_partial_history() {
    let rec = record_from_archive(&archive(&[(
        "x-2.0.0/CHANGELOG.md",
        "## 2.0.0\n- Removed `a::b`\n",
    )]));
    let card = summarize(&target(Ecosystem::Crates, "x", "1.0.0", "2.0.0"), &rec);
    assert_eq!(card.status, ReleaseChangesStatus::Found);
    assert!(
        !card.covers_range,
        "nothing at or below 1.0.0 in the changelog"
    );
    assert_eq!(card.breaking, 1);
}

#[test]
fn no_section_in_range_is_said_not_invented() {
    let rec = record_from_archive(&archive(&[("package/CHANGELOG.md", "## 1.0.0\n- old\n")]));
    let card = summarize(&target(Ecosystem::Npm, "pkg", "1.0.0", "3.0.0"), &rec);
    assert_eq!(card.status, ReleaseChangesStatus::NoSectionsInRange);
    assert_eq!(card.breaking + card.features + card.fixes + card.other, 0);
    assert!(card.reason.unwrap().contains("no section"));
    assert_eq!(
        card.source_url.as_deref(),
        Some("https://www.npmjs.com/package/pkg/v/3.0.0?activeTab=code")
    );
}

#[test]
fn an_archive_without_a_changelog_says_so() {
    let rec = record_from_archive(&archive(&[
        ("package/README.md", "# hi"),
        ("package/index.js", ""),
    ]));
    assert_eq!(rec.status, RecordStatus::NoChangelog);
    let card = summarize(&target(Ecosystem::Npm, "pkg", "1.0.0", "2.0.0"), &rec);
    assert_eq!(card.status, ReleaseChangesStatus::NoChangelog);
    assert!(card.top_breaking.is_empty());
}

#[test]
fn a_changelog_without_version_headings_is_unparsed() {
    let rec = record_from_archive(&archive(&[(
        "package/CHANGELOG.md",
        "See GitHub releases.\n",
    )]));
    assert_eq!(rec.status, RecordStatus::Unparsed);
    assert!(rec.reason.unwrap().contains("no version headings"));
}

#[test]
fn a_bad_archive_is_refused() {
    let rec = record_from_archive(b"<html>rate limited</html>");
    assert_eq!(rec.status, RecordStatus::Refused);
    let card = summarize(&target(Ecosystem::Crates, "x", "1.0.0", "2.0.0"), &rec);
    assert_eq!(card.status, ReleaseChangesStatus::Refused);
}

#[test]
fn top_breaking_is_capped() {
    let body = (0..12)
        .map(|i| format!("- Removed `f{i}`\n"))
        .collect::<Vec<_>>()
        .concat();
    let rec = record_from_archive(&archive(&[(
        "x-2.0.0/CHANGELOG.md",
        &format!("## 2.0.0\n{body}"),
    )]));
    let card = summarize(&target(Ecosystem::Crates, "x", "1.0.0", "2.0.0"), &rec);
    assert_eq!(card.breaking, 12);
    assert_eq!(card.top_breaking.len(), TOP_BREAKING);
}

static SEEDED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn seed(db: &Database, title: &str, source_type: &str, pins: &[(&str, &str, &str)]) -> i64 {
    let n = SEEDED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let conn = db.conn.lock();
    for (pkg, version, eco) in pins {
        conn.execute(
            "INSERT INTO user_dependencies (project_path, package_name, version, ecosystem, is_dev, is_direct, detected_at, last_seen_at)
             VALUES ('d:/proj/app', ?1, ?2, ?3, 0, 1, datetime('now'), datetime('now'))",
            rusqlite::params![pkg, version, eco],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO source_items (source_type, source_id, title, content, content_hash, embedding, created_at)
         VALUES (?1, ?2, ?3, '', ?2, zeroblob(4), datetime('now'))",
        rusqlite::params![source_type, format!("{title}#{n}"), title],
    )
    .unwrap();
    conn.last_insert_rowid()
}

#[test]
fn only_breaking_minor_and_yanked_rows_are_targets() {
    let db = crate::test_utils::test_db();
    seed(
        &db,
        "crates.io: rten v0.27.0",
        "crates_io",
        &[("rten", "0.24.0", "rust")],
    );
    seed(
        &db,
        "crates.io: tokio v1.53.2",
        "crates_io",
        &[("tokio", "1.52.3", "rust")],
    );
    seed(
        &db,
        "crates.io: heck v0.5.1",
        "crates_io",
        &[("heck", "0.5.0", "rust")],
    );
    seed(
        &db,
        "npm: stripe v23.0.0",
        "npm_registry",
        &[("stripe", "22.3.0", "javascript")],
    );
    let rten = release_target(&db, "crates_io", "crates.io: rten v0.27.0", "").unwrap();
    assert_eq!(rten, target(Ecosystem::Crates, "rten", "0.24.0", "0.27.0"));
    let tokio = release_target(&db, "crates_io", "crates.io: tokio v1.53.2", "").unwrap();
    assert_eq!(
        (tokio.from.as_str(), tokio.to.as_str()),
        ("1.52.3", "1.53.2")
    );
    assert_eq!(
        release_target(&db, "crates_io", "crates.io: heck v0.5.1", ""),
        None,
        "a patch is not news"
    );
    let stripe = release_target(&db, "npm_registry", "npm: stripe v23.0.0", "").unwrap();
    assert_eq!(stripe.eco, Ecosystem::Npm);
    assert_eq!(
        release_target(&db, "hackernews", "crates.io: rten v0.27.0", ""),
        None,
        "only registry rows"
    );
}

#[test]
fn the_backlog_skips_cached_archives_and_duplicate_rows() {
    let db = crate::test_utils::test_db();
    seed(
        &db,
        "crates.io: rten v0.27.0",
        "crates_io",
        &[("rten", "0.24.0", "rust")],
    );
    seed(&db, "crates.io: rten v0.27.0", "crates_io", &[]);
    seed(
        &db,
        "crates.io: lopdf v0.45.0",
        "crates_io",
        &[("lopdf", "0.42.0", "rust")],
    );
    let names: Vec<String> = backlog_targets(&db)
        .into_iter()
        .map(|t| t.package)
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let rec = record_from_archive(&archive(&[("rten-0.27.0/CHANGELOG.md", RTEN_CHANGELOG)]));
    store::put(&db, Ecosystem::Crates, "rten", "0.27.0", &rec).unwrap();
    let names: Vec<String> = backlog_targets(&db)
        .into_iter()
        .map(|t| t.package)
        .collect();
    assert_eq!(names, ["lopdf"]);
}

#[test]
fn the_backlog_takes_only_the_newest_release_per_package() {
    let db = crate::test_utils::test_db();
    seed(
        &db,
        "npm: ai v7.0.116",
        "npm_registry",
        &[("ai", "5.0.86", "javascript")],
    );
    seed(&db, "npm: ai v7.0.127", "npm_registry", &[]);
    seed(&db, "npm: ai v7.0.120", "npm_registry", &[]);
    let got: Vec<(String, String)> = backlog_targets(&db)
        .into_iter()
        .map(|t| (t.package, t.to))
        .collect();
    assert_eq!(got, [("ai".to_string(), "7.0.127".to_string())]);
}

#[test]
fn urls_stay_on_the_registry() {
    assert_eq!(
        fetch::crate_archive_url("rten", "0.27.0"),
        "https://static.crates.io/crates/rten/rten-0.27.0.crate"
    );
    assert!(fetch::is_archive_url(&fetch::crate_archive_url(
        "rten", "0.27.0"
    )));
    assert_eq!(
        fetch::npm_manifest_url("@types/node", "22.0.0"),
        "https://registry.npmjs.org/@types%2Fnode/22.0.0"
    );
    assert!(!fetch::is_archive_url("https://github.com/x/y/archive.tgz"));
    assert!(!fetch::is_archive_url(
        "http://registry.npmjs.org/x/-/x-1.0.0.tgz"
    ));
    assert!(!fetch::is_archive_url(
        "https://registry.npmjs.org:8443/x.tgz"
    ));
    assert!(!fetch::is_archive_url(
        "https://registry.npmjs.org.evil.example/x.tgz"
    ));
    let manifest = serde_json::json!({"dist": {"tarball": "https://registry.npmjs.org/stripe/-/stripe-23.0.0.tgz"}});
    assert_eq!(
        fetch::npm_tarball_from_manifest(&manifest).as_deref(),
        Some("https://registry.npmjs.org/stripe/-/stripe-23.0.0.tgz")
    );
    let elsewhere =
        serde_json::json!({"dist": {"tarball": "https://cdn.example.com/stripe-23.0.0.tgz"}});
    assert_eq!(fetch::npm_tarball_from_manifest(&elsewhere), None);
}

#[test]
fn names_and_versions_that_could_alter_a_url_are_rejected() {
    use fetch::{is_valid_package_name as name_ok, is_valid_version as ver_ok};
    assert!(name_ok("hyper-util", Ecosystem::Crates));
    assert!(!name_ok("../etc", Ecosystem::Crates));
    assert!(!name_ok("a/b", Ecosystem::Crates));
    assert!(name_ok("@tauri-apps/api", Ecosystem::Npm));
    assert!(name_ok("better-sqlite3", Ecosystem::Npm));
    assert!(!name_ok("@scope/../x", Ecosystem::Npm));
    assert!(!name_ok("x?y=1", Ecosystem::Npm));
    assert!(ver_ok("1.0.0-rc.1+build.5"));
    assert!(!ver_ok("1.0.0/../../x"));
    assert!(!ver_ok("latest"));
}

/// Live harness (network: static.crates.io + registry.npmjs.org only).
/// Point `FOURDA_CHANGELOG_LIVE_DB` at a COPY of the live database (never the
/// live file: the lane writes its cache). Prints one line per graded release:
/// `cargo test --lib release_changelog::tests::live_coverage -- --ignored --nocapture`
#[tokio::test]
#[ignore = "network + a live-database copy"]
async fn live_coverage() {
    let Ok(path) = std::env::var("FOURDA_CHANGELOG_LIVE_DB") else {
        return;
    };
    crate::register_sqlite_vec_extension();
    let db = Database::new(std::path::Path::new(&path)).unwrap();
    let mut seen = Vec::new();
    for (st, title, content) in recent_registry_rows(&db) {
        let Some(t) = release_target(&db, &st, &title, &content) else {
            continue;
        };
        if seen.contains(&(t.package.clone(), t.to.clone())) {
            continue;
        }
        seen.push((t.package.clone(), t.to.clone()));
        let card = match changelog_for(&db, &t).await {
            Ok(rec) => summarize(&t, &rec),
            Err(e) => unavailable(&t, e),
        };
        println!("LIVE {}", serde_json::to_string(&card).unwrap());
        tokio::time::sleep(FETCH_SPACING).await;
    }
}

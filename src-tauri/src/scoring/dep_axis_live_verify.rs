// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Live sweep of the dependency axis over a real corpus snapshot.
//!
//! The hermetic tests in `dependencies.rs` prove the rules on a fixed
//! adversarial corpus. This proves them on the operator's ACTUAL corpus, which
//! is where the 2026-08-26 audit found what no unit test could: 1,758 items
//! whose dependency axis was confirmed from the title alone, 1,320 of which
//! never named a package (75.1%), and 401 whose entire evidence set belonged to
//! a project this app does not ship.
//!
//! `#[ignore]`d because it needs a database. Point it at a SNAPSHOT, never the
//! live file — the run is read-only, but a scoring change should never be one
//! typo away from the only copy of the corpus:
//!
//! ```text
//! FOURDA_DB_PATH=/path/to/snapshot.db cargo test --lib \
//!     live_dep_axis_sweep -- --ignored --nocapture
//! ```
//!
//! Optional `FOURDA_VERIFY_CUTOFF` (an ISO timestamp) freezes the corpus at a
//! point in time so two runs are comparable while the engine keeps fetching.

use super::ace_context::ACEContext;
use super::dependencies::{load_dependency_intelligence, match_dependencies};

fn build_ctx() -> ACEContext {
    let (dependency_names, dependency_info) = load_dependency_intelligence();
    ACEContext {
        dependency_names,
        dependency_info,
        ..Default::default()
    }
}

fn band(score: f32) -> &'static str {
    if score >= 0.90 {
        ">=0.90"
    } else if score >= 0.70 {
        "0.70-0.89"
    } else if score >= 0.50 {
        "0.50-0.69"
    } else if score >= 0.30 {
        "0.30-0.49"
    } else {
        "0.20-0.29"
    }
}

#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_dep_axis_sweep() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        eprintln!("FOURDA_DB_PATH not set — nothing to verify");
        return;
    };
    let cutoff = std::env::var("FOURDA_VERIFY_CUTOFF").unwrap_or_else(|_| "9999".to_string());
    println!("snapshot: {path}\ncutoff:   {cutoff}");

    let ctx = build_ctx();
    let foreign = ctx
        .dependency_info
        .values()
        .filter(|d| d.project_paths.iter().all(|p| !p.contains("4da")))
        .count();
    println!(
        "packages: {}  search terms: {}  scope degraded: {}  non-4DA packages: {foreign}",
        ctx.dependency_info.len(),
        ctx.dependency_names.len(),
        crate::temporal::dep_scope_degraded(),
    );

    let conn = rusqlite::Connection::open(&path).expect("open snapshot");
    let mut stmt = conn
        .prepare("SELECT title, COALESCE(content,'') FROM source_items WHERE created_at < ?1")
        .expect("prepare");
    let rows: Vec<(String, String)> = stmt
        .query_map([&cutoff], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("query")
        .filter_map(|r| r.ok())
        .collect();
    println!("corpus items: {}", rows.len());

    for (label, use_content) in [("TITLE-ONLY", false), ("TITLE+CONTENT", true)] {
        let (mut confirmed, mut phantom, mut foreign_only, mut bypass) = (0usize, 0, 0, 0);
        let mut bands: std::collections::BTreeMap<&str, usize> = Default::default();
        let mut kept: Vec<(f32, String, String)> = Vec::new();

        for (title, content) in &rows {
            let body = if use_content { content.as_str() } else { "" };
            let (matches, score) = match_dependencies(title, body, &[], &ctx);
            if score < crate::scoring_config::DEPENDENCY_THRESHOLD {
                continue;
            }
            confirmed += 1;
            if !matches.iter().any(|m| m.corroborated) {
                phantom += 1;
            }
            if !matches.is_empty()
                && matches.iter().all(|m| {
                    !m.project_paths.is_empty()
                        && m.project_paths.iter().all(|p| !p.contains("4da"))
                })
            {
                foreign_only += 1;
            }
            if score >= crate::scoring_config::DEPENDENCY_GATE_BYPASS_DIRECT_DEP_MIN_SCORE {
                bypass += 1;
                if kept.len() < 15 {
                    let ev = matches
                        .iter()
                        .map(|m| m.package_name.clone())
                        .collect::<Vec<_>>()
                        .join(",");
                    kept.push((score, ev, title.chars().take(88).collect()));
                }
            }
            *bands.entry(band(score)).or_default() += 1;
        }

        println!("\n===== {label} =====");
        println!("  confirmed (>= threshold): {confirmed}");
        println!("  uncorroborated (phantom): {phantom}");
        println!("  foreign-project-only:     {foreign_only}");
        println!("  clears the gate bypass:   {bypass}");
        for (b, n) in bands.iter().rev() {
            println!("    {b:<10} {n:>6}");
        }
        for (s, ev, t) in &kept {
            println!("    [{s:.2}] ({ev}) {t}");
        }

        // The invariants the audit's remediation exists to hold. These are
        // assertions, not prints: a regression here is the feed filling with
        // plausible-looking noise again, which is precisely what survived
        // casual inspection for months.
        assert_eq!(
            phantom, 0,
            "{label}: an item that names NO package the user depends on must never confirm the dependency axis"
        );
        assert_eq!(
            foreign_only, 0,
            "{label}: a dependency belonging only to a project outside the user's active roots must never carry an item"
        );
    }
}

/// A6: the scanner discards every parsed version at the manifest write, so the
/// SameMajor / NewerMajor / OlderMajor multipliers have never fired in
/// production. Reports the live coverage so the backfill can be verified.
#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database snapshot"]
fn live_version_coverage() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        return;
    };
    let ctx = build_ctx();
    let total = ctx.dependency_info.len();
    let with_version = ctx
        .dependency_info
        .values()
        .filter(|d| d.version.is_some())
        .count();
    println!("DepInfo entries: {total}, carrying a version: {with_version}");

    let conn = rusqlite::Connection::open(&path).expect("open");
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM project_dependencies WHERE version IS NOT NULL AND version <> ''",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    println!("project_dependencies rows with a version: {n}");

    let mut found = 0usize;
    let mut stmt = conn
        .prepare(
            "SELECT 1 FROM user_dependencies \
             WHERE LOWER(package_name) = ? AND version IS NOT NULL AND version <> '' LIMIT 1",
        )
        .expect("prepare");
    for info in ctx.dependency_info.values() {
        if stmt
            .exists([info.package_name.to_lowercase()])
            .unwrap_or(false)
        {
            found += 1;
        }
    }
    println!(
        "recoverable from user_dependencies: {found} / {total} ({:.0}%)",
        found as f64 * 100.0 / total.max(1) as f64
    );
}

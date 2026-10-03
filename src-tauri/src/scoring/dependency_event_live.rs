// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Live sweep of the dependency-event claim over a real corpus.
//!
//! The flag is new, so no stored breakdown carries it. This replays the claim
//! over the items whose STORED breakdown is strongly grounded — the current
//! "Affects You" pool's grounding route — using only what the scorer stored
//! with the score: the matched dependencies, the version verdict, and the
//! release grade's explanation line. It prints one JSON line per item so the
//! result can be joined against an external label set.
//!
//! Approximations (the full scorer state is not persisted):
//! - registry advisories: `is_version_affected` true → affected, false → not
//!   affected, unknown on a grounded row → likely affected (the scorer's
//!   `security_applicability` route for a grounded advisory);
//! - registry releases: the class is read back from the stored grade line
//!   ("Breaking upgrade…", "New minor…", "Patch release…", "Pre-release…",
//!   "Your pinned … was yanked", "Your own package…");
//! - editorial items: every stored matched dependency is treated as a strong
//!   grounding candidate (the stored set is the display-worthy, corroborated
//!   one), with the manifest's written name restored from `user_dependencies`.
//!
//! `#[ignore]`d; read-only. Point it at the live file or a snapshot:
//!
//! ```text
//! FOURDA_DB_PATH="file:D:/4DA/data/4da.db?mode=ro" cargo test --lib \
//!     live_dependency_event_sweep -- --ignored --nocapture
//! ```

use std::collections::HashMap;

use super::dependencies::{normalize_package_name, DepMatch, VersionDelta};
use super::dependency_event::{is_dependency_event, EventInputs};
use super::release_grade::ReleaseClass;

/// The release class and "not news" verdict from a stored grade line.
fn class_from_grade_line(line: &str) -> (Option<ReleaseClass>, bool) {
    if line.starts_with("Breaking upgrade") {
        (Some(ReleaseClass::Breaking), false)
    } else if line.starts_with("New minor") || line.starts_with("New release of") {
        (Some(ReleaseClass::Minor), false)
    } else if line.starts_with("Your pinned") && line.contains("yanked") {
        (Some(ReleaseClass::Yanked), false)
    } else if line.starts_with("Patch release") {
        (Some(ReleaseClass::Patch), false)
    } else if line.starts_with("Pre-release") {
        (Some(ReleaseClass::Prerelease), false)
    } else if line.starts_with("Your own package") {
        (None, true)
    } else {
        (None, false)
    }
}

fn dep(normalized: &str, raw: Option<&String>) -> DepMatch {
    DepMatch {
        package_name: normalized.to_string(),
        confidence: 0.5,
        version_delta: VersionDelta::Unknown,
        is_dev: false,
        is_direct: true,
        version: None,
        ecosystem: String::new(),
        corroborated: true,
        project_paths: Vec::new(),
        raw_name: raw.cloned(),
    }
}

#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a real database"]
fn live_dependency_event_sweep() {
    let Ok(path) = std::env::var("FOURDA_DB_PATH") else {
        eprintln!("FOURDA_DB_PATH not set — nothing to verify");
        return;
    };
    let days = std::env::var("FOURDA_EVENT_DAYS").unwrap_or_else(|_| "14".to_string());
    let conn = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .expect("open read-only");

    let mut raw_names: HashMap<String, String> = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT DISTINCT package_name FROM user_dependencies")
        .expect("deps");
    for name in stmt
        .query_map([], |r| r.get::<_, String>(0))
        .expect("deps")
        .flatten()
    {
        raw_names.insert(normalize_package_name(&name), name);
    }

    let sql = format!(
        "SELECT si.id, si.source_type, si.title, COALESCE(si.content, ''),
                COALESCE(si.feed_relevant, -1),
                json_extract(e.breakdown, '$.breakdown.matched_deps'),
                json_extract(e.breakdown, '$.breakdown.is_version_affected'),
                COALESCE(json_extract(e.breakdown, '$.breakdown.explanation_factors[0].display'), '')
         FROM source_items si JOIN scoring_explanations e ON e.source_item_id = si.id
         WHERE json_extract(e.breakdown, '$.breakdown.strongly_grounded') = 1
           AND si.created_at > datetime('now', '-{days} days')
         ORDER BY si.id"
    );
    let mut stmt = conn.prepare(&sql).expect("query");
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, String>(7)?,
            ))
        })
        .expect("rows");

    let (mut total, mut events) = (0usize, 0usize);
    for row in rows.flatten() {
        let (id, source_type, title, content, feed, deps_json, affected, line) = row;
        let names: Vec<String> = deps_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        let deps: Vec<DepMatch> = names
            .iter()
            .map(|n| dep(n, raw_names.get(&normalize_package_name(n))))
            .collect();
        let registry_advisory = matches!(source_type.as_str(), "osv" | "cve");
        let applicability = match affected {
            Some(1) => Some("affected"),
            Some(_) => Some("not_affected"),
            None => Some("likely_affected"),
        };
        let registry = crate::dep_linker::is_registry_source(&source_type);
        let (release_class, own) = class_from_grade_line(&line);
        let event = is_dependency_event(&EventInputs {
            source_type: &source_type,
            title: &title,
            content: &content,
            registry_advisory,
            security_confirmed: false,
            applicability,
            via_registry_subject: registry,
            release_class,
            already_installed_release: own,
            strongly_grounded: true,
            deps: &deps,
        });
        total += 1;
        events += usize::from(event);
        println!(
            "{}",
            serde_json::json!({
                "id": id,
                "source_type": source_type,
                "feed_relevant": feed,
                "dependency_event": event,
                "matched_deps": names,
                "title": title,
                "line": line,
            })
        );
    }
    println!("SUMMARY grounded={total} dependency_event={events}");
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Registry releases graded against what is installed, for the stack-change
//! stream. The grading is `scoring::release_grade` (the rules the feed scorer,
//! the Brief and knowledge gaps share); this module only picks the latest
//! stable release per package inside the Brief's window, keeps the grades that
//! ask something of a project, and reads whether a minor release is inside
//! the range the project already declares.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use semver::Version;

use crate::brief_facts::{is_own_package, UPGRADE_WINDOW_DAYS};
use crate::db::Database;
use crate::evidence::ProjectLiveness;
use crate::osv::fix_path::{cargo_requirements, npm_requirement, Manager, Requirement};
use crate::scoring::release_grade::{self, ProjectPin, ReleaseClass, ReleaseGrade};

use super::StackChange;

/// Registry rows read per pass (newest first). The Brief reads the same cap.
const ROW_LIMIT: usize = 3000;

/// One project that a release asks something of.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReleaseSite {
    pub project_path: String,
    pub label: String,
    pub installed: String,
    pub is_dev: bool,
    /// The refresh command that takes this project to the release, named only
    /// when its own manifest's requirement was READ and admits the release.
    pub command: Option<String>,
    /// What the project's manifest requirement says about the release:
    /// `Some(true)` admits it, `Some(false)` excludes it, `None` not read.
    pub admitted: Option<bool>,
}

/// A graded release that changes something for at least one active project.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReleaseChange {
    pub package: String,
    /// OSV-canonical ecosystem name ("crates.io", "npm", "PyPI", "Go").
    pub ecosystem: String,
    pub change: StackChange,
    pub announced: String,
    /// `YYYY-MM-DD`, when the registry row carries a date.
    pub published: Option<String>,
    pub url: Option<String>,
    pub source_type: String,
    pub row_title: String,
    pub sites: Vec<ReleaseSite>,
    /// Projects that already run the release or newer, as a label.
    pub current: Option<String>,
    /// Every concerned project declares it as a dev dependency.
    pub dev_only: bool,
}

/// The change a grade asks for, or `None` when it asks nothing of a project.
/// Patches are not news on their own (security fixes arrive through the
/// advisory lane) and neither are prereleases (`release_grade`'s rules).
pub(crate) fn change_of(grade: &ReleaseGrade) -> Option<StackChange> {
    match grade.class()? {
        ReleaseClass::Yanked => Some(StackChange::Yanked),
        // Below 1.0 every minor is a breaking release (Cargo's unit).
        ReleaseClass::Breaking if grade.announced.major == 0 => Some(StackChange::Breaking),
        ReleaseClass::Breaking => Some(StackChange::Major),
        ReleaseClass::Minor => Some(StackChange::Minor),
        ReleaseClass::Patch | ReleaseClass::Prerelease => None,
    }
}

/// Whether one project's manifest requirement on `package` admits `version`.
/// Every requirement read must admit it; any that excludes it says no.
pub(crate) fn requirements_admit(requirements: &[Requirement], version: &str) -> Option<bool> {
    let verdicts: Vec<Option<bool>> = requirements.iter().map(|r| r.admits(version)).collect();
    if verdicts.is_empty() {
        return None;
    }
    if verdicts.contains(&Some(false)) {
        return Some(false);
    }
    verdicts.iter().all(|v| *v == Some(true)).then_some(true)
}

/// The requirement the project at `dir` declares on `package` in its own
/// manifest, read with the parsers the fix-path rule uses. Empty when the
/// manifest is absent or names it another way (a workspace-inherited crate).
fn own_manifest_requirements(dir: &Path, ecosystem: &str, package: &str) -> Vec<Requirement> {
    match ecosystem {
        "crates.io" => std::fs::read_to_string(dir.join("Cargo.toml"))
            .map(|manifest| {
                cargo_requirements(&manifest, package)
                    .iter()
                    .map(|r| Requirement::cargo(r))
                    .collect()
            })
            .unwrap_or_default(),
        "npm" => std::fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|pkg| npm_requirement(&pkg, package))
            .map(|r| vec![Requirement::npm(&r)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn site_for(
    pin: &ProjectPin,
    change: StackChange,
    ecosystem: &str,
    grade: &ReleaseGrade,
) -> ReleaseSite {
    let dir = Path::new(&pin.project_path);
    let installed = pin.installed.to_string();
    let announced = grade.announced.to_string();
    // A breaking release always needs a manifest edit; only a minor can land
    // by refreshing inside the range the project already declares.
    let admitted = if change == StackChange::Minor && !pin.is_dev {
        requirements_admit(
            &own_manifest_requirements(dir, ecosystem, &grade.package),
            &announced,
        )
    } else {
        None
    };
    let command = (admitted == Some(true))
        .then(|| Manager::detect(dir, ecosystem))
        .flatten()
        .and_then(|m| m.refresh_command(&grade.package, &installed, &announced));
    ReleaseSite {
        project_path: pin.project_path.clone(),
        label: release_grade::project_label(std::slice::from_ref(&pin.project_path))
            .unwrap_or_else(|| pin.project_path.clone()),
        installed,
        is_dev: pin.is_dev,
        command,
        admitted,
    }
}

/// Build the change for one graded release, or `None` when it asks nothing of
/// an active project. Scratch and dormant projects are not working projects
/// (the Brief's rule); a minor that only dev tooling carries is tooling news.
pub(crate) fn release_change(
    grade: &ReleaseGrade,
    row: &RegistryRow,
    liveness: &ProjectLiveness,
) -> Option<ReleaseChange> {
    let change = change_of(grade)?;
    let ecosystem = crate::osv::exposure::registry_source_ecosystem(&row.source_type)?;
    let active: Vec<&ProjectPin> = grade
        .concerned_pins()
        .into_iter()
        .filter(|p| !liveness.is_scratch(&p.project_path) && !liveness.is_dormant(&p.project_path))
        .collect();
    if active.is_empty() {
        return None;
    }
    let dev_only = active.iter().all(|p| p.is_dev);
    if dev_only && change == StackChange::Minor {
        return None;
    }
    let mut sites: Vec<ReleaseSite> = active
        .iter()
        .map(|p| site_for(p, change, ecosystem, grade))
        .collect();
    sites.sort_by(|a, b| a.label.cmp(&b.label).then(a.installed.cmp(&b.installed)));
    sites.dedup_by(|a, b| a.project_path == b.project_path);
    Some(ReleaseChange {
        package: grade.package.clone(),
        ecosystem: ecosystem.to_string(),
        change,
        announced: grade.announced.to_string(),
        published: row.published.as_ref().map(|p| p.chars().take(10).collect()),
        url: row.url.clone(),
        source_type: row.source_type.clone(),
        row_title: row.title.clone(),
        sites,
        current: grade.current_label(),
        dev_only,
    })
}

/// One registry release row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RegistryRow {
    pub source_type: String,
    pub title: String,
    pub content: String,
    pub url: Option<String>,
    pub published: Option<String>,
}

fn load_rows(conn: &rusqlite::Connection) -> Vec<RegistryRow> {
    let placeholders = crate::dep_linker::REGISTRY_SOURCE_TYPES
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT source_type, title, COALESCE(content, ''), url, COALESCE(published_at, created_at)
         FROM source_items
         WHERE source_type IN ({placeholders})
           AND COALESCE(published_at, created_at) >= datetime('now', ?1)
         ORDER BY id DESC LIMIT {ROW_LIMIT}"
    );
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let window = format!("-{UPGRADE_WINDOW_DAYS} days");
    stmt.query_map(rusqlite::params![window], |r| {
        Ok(RegistryRow {
            source_type: r.get(0)?,
            title: r.get(1)?,
            content: r.get(2)?,
            url: r.get(3)?,
            published: r.get(4)?,
        })
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// Detected project names: the packages the user publishes from their own
/// workspaces (the Brief's own-package guard), in `is_own_package`'s form.
fn own_package_names(conn: &rusqlite::Connection) -> HashSet<String> {
    let Ok(mut stmt) = conn.prepare("SELECT name FROM detected_projects") else {
        return HashSet::new();
    };
    stmt.query_map([], |r| r.get::<_, String>(0))
        .map(|rows| {
            rows.flatten()
                .map(|n| n.trim().to_lowercase().replace('_', "-"))
                .collect()
        })
        .unwrap_or_default()
}

/// Every registry release from the Brief's window that changes something for
/// an active project: the latest stable release per package, graded against
/// each project's pin.
pub(crate) fn collect_release_changes(
    db: &Database,
    conn: &rusqlite::Connection,
) -> Vec<ReleaseChange> {
    let own = own_package_names(conn);
    let liveness = ProjectLiveness::load(conn);
    // (registry family, package) -> (announced, grade, row): the highest
    // STABLE release wins, so a 4.0.0-alpha row never hides 3.2.0.
    let mut best: BTreeMap<(String, String), (Version, ReleaseGrade, RegistryRow)> =
        BTreeMap::new();
    for row in load_rows(conn) {
        let Some(grade) =
            release_grade::grade_registry_release(db, &row.source_type, &row.title, &row.content)
        else {
            continue;
        };
        if !grade.announced.pre.is_empty() || is_own_package(&grade.package, &own) {
            continue;
        }
        let Some(eco) = crate::osv::exposure::registry_source_ecosystem(&row.source_type) else {
            continue;
        };
        let key = (
            eco.to_string(),
            grade.package.to_lowercase().replace('_', "-"),
        );
        let newer = best
            .get(&key)
            .is_none_or(|(announced, ..)| grade.announced > *announced);
        if newer {
            best.insert(key, (grade.announced.clone(), grade, row));
        }
    }
    best.into_values()
        .filter_map(|(_, grade, row)| release_change(&grade, &row, &liveness))
        .collect()
}

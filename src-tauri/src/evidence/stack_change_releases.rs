// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Registry releases graded against what is installed, for the stack-change
//! stream. The grading is `scoring::release_grade` (the rules the feed scorer,
//! the Brief and knowledge gaps share); this module only picks the newest
//! stable release per package inside the Brief's window, keeps the grades that
//! ask something of a project, and reads whether a minor release is inside
//! the range the project already declares.
//!
//! Before any command toward a release is named, the release itself is
//! checked against the package's stored advisories
//! (`osv::release_advisories`): a malicious or advisory-affected release is
//! announced as such with no command, and a release whose advisory status is
//! unknown (the mirror is stale, failed, or older than the release) gets no
//! command either. A release affected only by advisories that already hit the
//! copies the projects run is left to the advisory lane, which reports those
//! advisories with a target that clears them.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use semver::Version;

use crate::brief_facts::{is_own_package, UPGRADE_WINDOW_DAYS};
use crate::db::Database;
use crate::evidence::ProjectLiveness;
use crate::osv::fix_path::{cargo_requirements, Manager, Requirement};
use crate::osv::release_advisories::{ReleaseAdvisories, ReleaseChecker, RELEASE_HOLD_DAYS};
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
    /// The change the release's grade alone asks for (`None` for a patch).
    /// Differs from `change` only for an affected release, which is a
    /// `Security` change whatever its size; it ranks a non-malicious one.
    pub graded: Option<StackChange>,
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
    /// What the stored advisories say about the announced release itself.
    /// Commands are named only when it is `Clear`.
    pub advisories: ReleaseAdvisories,
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
        "npm" => npm_manifest_requirement(dir, package).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// The spec a project's own `package.json` declares `package` with, in any
/// section the project installs (`dependencies`, `devDependencies`,
/// `optionalDependencies`; `direct_upgrade::npm_declared`): a project's own
/// dev dependencies resolve, and their install scripts run.
fn npm_manifest_requirement(dir: &Path, package: &str) -> Option<Requirement> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    crate::osv::direct_upgrade::npm_declared(&text, package).map(|r| Requirement::npm(&r))
}

/// What the project's own manifest AND, for an npm-family workspace root,
/// its members' manifests declare on `package`: the workspace's one
/// lockfile resolves every member's requirement, so a refresh there pulls
/// in a release any of them admits.
fn workspace_requirements(dir: &Path, ecosystem: &str, package: &str) -> Vec<Requirement> {
    let mut out = own_manifest_requirements(dir, ecosystem, package);
    if ecosystem == "npm" {
        out.extend(
            crate::ace::npm_local::workspace_member_dirs(dir)
                .iter()
                .filter_map(|m| npm_manifest_requirement(m, package)),
        );
    }
    out
}

fn site_for(
    pin: &ProjectPin,
    change: StackChange,
    ecosystem: &str,
    grade: &ReleaseGrade,
    clear: bool,
) -> ReleaseSite {
    let dir = Path::new(&pin.project_path);
    let installed = pin.installed.to_string();
    let announced = grade.announced.to_string();
    // A breaking release always needs a manifest edit; only a minor (or, for
    // an affected release, any size) can land by refreshing inside the range
    // the project (or a workspace member) declares. Read for dev pins too:
    // install scripts run for them, so a malicious dev release inside the
    // range is as close to installed.
    let admitted = matches!(change, StackChange::Minor | StackChange::Security).then(|| {
        requirements_admit(
            &workspace_requirements(dir, ecosystem, &grade.package),
            &announced,
        )
    });
    let admitted = admitted.flatten();
    // A command only for a runtime pin whose OWN manifest admits a release
    // checked clear: the refresh that "picks it up" is the one that would
    // install a malicious build, and the root's refresh command is only
    // known to act on the root's own declaration.
    let own_admits = requirements_admit(
        &own_manifest_requirements(dir, ecosystem, &grade.package),
        &announced,
    ) == Some(true);
    let command = (admitted == Some(true) && own_admits && clear && !pin.is_dev)
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

/// The grade over WORKING projects only: scratch and dormant projects are
/// not working projects (the Brief's rule). Narrowed BEFORE grading
/// (`ReleaseGrade::retain_projects`), so the largest gap is the largest an
/// active project has, not one a scratch tree has.
pub(crate) fn active_grade(grade: &ReleaseGrade, liveness: &ProjectLiveness) -> ReleaseGrade {
    grade
        .clone()
        .retain_projects(|p| !liveness.is_scratch(p) && !liveness.is_dormant(p))
}

/// The projects an active grade concerns that declare the package directly
/// (the only copies a release row can ask something of): for an unaffected
/// release, the ones with the largest gap, which set its class.
pub(crate) fn active_pins(active: &ReleaseGrade) -> Vec<&ProjectPin> {
    active
        .concerned_pins()
        .into_iter()
        .filter(|p| p.is_direct)
        .collect()
}

/// EVERY active project that declares the package directly and is behind
/// the release: the sites of an AFFECTED release, and the copies its
/// coverage is judged on. Not the largest-gap set: for 4.2.1 with A on
/// 4.1.0 and B on 4.2.0 that set is A alone, so a malicious 4.2.1 never
/// named B (whose caret range is the closest to pulling it in), and an
/// advisory hitting 4.2.1 and A's copy counted as covered and dropped the
/// warning B needed.
pub(crate) fn behind_pins(active: &ReleaseGrade) -> Vec<&ProjectPin> {
    active
        .pins
        .iter()
        .filter(|p| p.is_direct && p.gap.is_some())
        .collect()
}

/// Build the change for one graded release, or `None` when it asks nothing of
/// an active project. Graded over the active projects alone ([`active_grade`]);
/// see [`release_change_active`]. The collector narrows once and calls that
/// directly; this is the tests' entry point.
#[cfg(test)]
pub(crate) fn release_change(
    grade: &ReleaseGrade,
    row: &RegistryRow,
    liveness: &ProjectLiveness,
    advisories: ReleaseAdvisories,
) -> Option<ReleaseChange> {
    release_change_active(&active_grade(grade, liveness), row, advisories)
}

/// [`release_change`] for a grade already narrowed to working projects.
/// A minor that only dev tooling carries is tooling news, unless an advisory
/// affects it. An affected release is a SECURITY change whatever its size: a
/// malicious patch inside a caret range is the one a refresh installs first,
/// and "patches are not news" must not hide it; its sites are every active
/// project behind it ([`behind_pins`]). A release whose every affecting
/// advisory already hits every such copy is the advisory lane's (it names
/// the target that clears them), never a malicious one. `advisories` must be
/// checked against the [`behind_pins`]' copies.
pub(crate) fn release_change_active(
    grade: &ReleaseGrade,
    row: &RegistryRow,
    advisories: ReleaseAdvisories,
) -> Option<ReleaseChange> {
    let affected = match &advisories {
        ReleaseAdvisories::Affected(hit) if hit.covered_by_installed && !hit.malware => {
            return None
        }
        ReleaseAdvisories::Affected(_) => true,
        _ => false,
    };
    let graded = change_of(grade);
    let change = if affected {
        StackChange::Security
    } else {
        graded?
    };
    let ecosystem = crate::osv::exposure::registry_source_ecosystem(&row.source_type)?;
    let active = if affected {
        behind_pins(grade)
    } else {
        active_pins(grade)
    };
    if active.is_empty() {
        return None;
    }
    let dev_only = active.iter().all(|p| p.is_dev);
    if dev_only && change == StackChange::Minor && !affected {
        return None;
    }
    let mut sites: Vec<ReleaseSite> = active
        .iter()
        .map(|p| site_for(p, change, ecosystem, grade, advisories.is_clear()))
        .collect();
    sites.sort_by(|a, b| a.label.cmp(&b.label).then(a.installed.cmp(&b.installed)));
    sites.dedup_by(|a, b| a.project_path == b.project_path);
    Some(ReleaseChange {
        package: grade.package.clone(),
        ecosystem: ecosystem.to_string(),
        change,
        graded,
        announced: grade.announced.to_string(),
        published: row.published.as_ref().map(|p| p.chars().take(10).collect()),
        url: row.url.clone(),
        source_type: row.source_type.clone(),
        row_title: row.title.clone(),
        sites,
        current: grade.current_label(),
        dev_only,
        advisories,
    })
}

// ---------------------------------------------------------------------------
// What the advisory check adds to a release item (`stack_change::release_item`)
// ---------------------------------------------------------------------------

/// "MAL-2026-1 and GHSA-x", "A, B and 2 more".
fn id_list(ids: &[String]) -> String {
    let shown: Vec<&str> = ids.iter().take(3).map(String::as_str).collect();
    let more = ids.len() - shown.len();
    match (shown.as_slice(), more) {
        ([one], 0) => (*one).to_string(),
        (_, 0) => {
            let (last, head) = shown.split_last().unwrap_or((&"", &[]));
            format!("{} and {last}", head.join(", "))
        }
        _ => format!("{} and {more} more", shown.join(", ")),
    }
}

/// The projects whose declared requirement admits the release: a lockfile
/// refresh there (or an install without the lockfile) would resolve to it.
fn admitting(r: &ReleaseChange) -> Vec<&str> {
    r.sites
        .iter()
        .filter(|s| s.admitted == Some(true))
        .map(|s| s.label.as_str())
        .collect()
}

/// The title for a release the advisories speak against, replacing the
/// "new minor" one.
pub(crate) fn advisory_title(r: &ReleaseChange) -> Option<String> {
    let ReleaseAdvisories::Affected(hit) = &r.advisories else {
        return None;
    };
    let (pkg, new) = (&r.package, &r.announced);
    Some(if hit.malware {
        format!("{pkg} {new}: malicious release, do not install")
    } else {
        format!("{pkg} {new}: affected by {}", id_list(&hit.ids))
    })
}

/// What to do about a release the advisories speak against or the registry
/// withdrew, replacing the upgrade advice. `None` for a clear or unchecked
/// release.
pub(crate) fn advisory_action(r: &ReleaseChange) -> Option<String> {
    let (pkg, new) = (&r.package, &r.announced);
    let admitting = admitting(r);
    let pulled_in = (!admitting.is_empty()).then(|| {
        format!(
            " {} declare{} a requirement that admits {new}, so a lockfile refresh, or an install \
             without the lockfile, would pull it in: keep the version the lockfile pins and do not \
             update {pkg} until a release no advisory affects is out.",
            admitting.join(", "),
            if admitting.len() == 1 { "s" } else { "" }
        )
    });
    let text = match &r.advisories {
        ReleaseAdvisories::Affected(hit) if hit.malware => format!(
            "OSV lists {pkg} {new} as a malicious release ({}): do not install it.",
            id_list(&hit.ids)
        ),
        ReleaseAdvisories::Affected(hit) => format!(
            "{pkg} {new} is {} by {}, so 4DA names no command toward it: stay on the installed \
             version, or move to a release no advisory affects.",
            if hit.confirmed {
                "affected"
            } else {
                "possibly affected (4DA could not place it outside the affected range)"
            },
            id_list(&hit.ids)
        ),
        ReleaseAdvisories::Withdrawn => format!(
            "The registry withdrew {pkg} {new} (yanked, or deprecated in favour of a later \
             release), so 4DA names no command toward it."
        ),
        ReleaseAdvisories::Clear
        | ReleaseAdvisories::Unknown(_)
        | ReleaseAdvisories::Held { .. } => return None,
    };
    Some(format!("{text}{}", pulled_in.unwrap_or_default()))
}

/// The sentence for a release 4DA could not check, or one too new to
/// trust: never presented as clear.
pub(crate) fn unchecked_note(r: &ReleaseChange) -> Option<String> {
    match &r.advisories {
        ReleaseAdvisories::Unknown(reason) => Some(format!(
            "Advisory status unknown: 4DA has not checked {} {} against vulnerability \
             advisories, because {}, so it names no command toward it.",
            r.package,
            r.announced,
            reason.words()
        )),
        ReleaseAdvisories::Held { published, days } => Some(format!(
            "{} {} was published {} ({published}); 4DA names no command for a release younger \
             than {RELEASE_HOLD_DAYS} days, so a compromised publish can be caught and pulled first.",
            r.package,
            r.announced,
            match days {
                0 => "today".to_string(),
                1 => "1 day ago".to_string(),
                n => format!("{n} days ago"),
            }
        )),
        _ => None,
    }
}

/// The advisory ids to cite for an affected release.
pub(crate) fn advisory_ids(r: &ReleaseChange) -> &[String] {
    match &r.advisories {
        ReleaseAdvisories::Affected(hit) => &hit.ids,
        _ => &[],
    }
}

/// The release is a malicious build: as urgent as a yanked pin, because
/// inside a declared range it is one lockfile refresh away from installed.
pub(crate) fn is_malicious(r: &ReleaseChange) -> bool {
    matches!(&r.advisories, ReleaseAdvisories::Affected(hit) if hit.malware)
}

/// The change that ranks the item and sets its base urgency. A malicious
/// release is security news first; any other affected release ranks where
/// its grade puts it (a patch with the minors), so it never outranks a
/// yanked pin or loses a major's urgency by being badged `security`.
pub(crate) fn rank_change(r: &ReleaseChange) -> StackChange {
    match r.change {
        StackChange::Security if !is_malicious(r) => r.graded.unwrap_or(StackChange::Minor),
        change => change,
    }
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
    // `published` is the registry's own date or nothing: the fetch time
    // (`created_at`) read as "released <the day 4DA saw it>" (audit E5). The
    // window still falls back to it so an undated release is not lost.
    let sql = format!(
        "SELECT source_type, title, COALESCE(content, ''), url, published_at
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
/// an active project: the newest stable release per package, graded against
/// each working project's pin.
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
    let checker = ReleaseChecker::load(db);
    best.into_values()
        .filter_map(|(_, grade, row)| {
            // Graded and judged on the WORKING projects alone: a scratch copy
            // must neither set the class nor stand in for an active copy.
            // Every release an active project is behind on is checked, a
            // patch included: an affected one surfaces whatever its size.
            let active = active_grade(&grade, &liveness);
            let installed: Vec<String> = behind_pins(&active)
                .iter()
                .map(|p| p.installed.to_string())
                .collect();
            if installed.is_empty() {
                return None;
            }
            let verdict = checker.check_grade(
                db,
                &row.source_type,
                &active,
                &installed,
                row.published.as_deref(),
            );
            release_change_active(&active, &row, verdict)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    /// Fresh-profile audit E5: "released <date>" showed the day 4DA fetched
    /// the row when the registry gave no publish date. Only the registry's
    /// date is shown; without one there is no date (the window still counts
    /// the fetch time, so the release is not lost).
    #[test]
    fn a_release_without_a_registry_date_shows_no_date_not_the_fetch_date() {
        let db = crate::test_utils::test_db();
        let conn = db.conn.lock();
        for (id, published) in [
            ("undated@2.0.0", None),
            ("dated@3.0.0", Some("2026-10-01 10:00:00")),
        ] {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content, content_hash,
                     embedding, published_at)
                 VALUES ('crates_io', ?1, ?1, '', ?1, x'', ?2)",
                rusqlite::params![id, published],
            )
            .unwrap();
        }
        let rows = super::load_rows(&conn);
        let published = |t: &str| {
            rows.iter()
                .find(|r| r.title == t)
                .map(|r| r.published.clone())
        };
        assert_eq!(published("undated@2.0.0"), Some(None), "{rows:?}");
        assert_eq!(
            published("dated@3.0.0"),
            Some(Some("2026-10-01 10:00:00".to_string()))
        );
    }
}

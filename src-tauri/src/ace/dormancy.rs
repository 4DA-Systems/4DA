// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Project dormancy — how long since a detected project last saw real activity.
//!
//! `detected_projects.last_activity` used to be stamped with the SCAN clock
//! (`signal.detected_at` is `Utc::now()` at scan time), so every rescan marked
//! every project — including repos untouched since February — as active
//! "today". The 2026-08-31 live audit traced every critical Preemption upgrade
//! card and the AI briefing's "Action Required" nags to exactly those
//! graveyard projects. This module supplies the real signal: filesystem
//! evidence of the last time the project itself moved (git activity markers
//! and manifest mtimes), plus the pure helpers downstream policy uses to ask
//! "is this project dormant, and for how long?".

use std::path::Path;
use std::time::SystemTime;

use super::scanner::resolve_git_dir;

/// A project with no activity for more than this many days is dormant.
/// Dormant projects still get alerts, but never Critical/High urgency —
/// see `crate::evidence::liveness`.
pub const DORMANT_AFTER_DAYS: i64 = 90;

/// Manifest files whose mtime counts as project activity. Editing any of
/// these (or committing — see the git markers) is what "working on the
/// project" looks like on disk. Lockfiles are included: an install/update
/// touches the lockfile even when the manifest is untouched.
const ACTIVITY_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "go.sum",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    "composer.json",
];

/// Days since `last_activity`. Accepts RFC3339 (what the scanner writes) and
/// the SQLite default `YYYY-MM-DD HH:MM:SS` (defensive: older rows and manual
/// edits). Returns `None` for empty/unparseable values — callers MUST treat
/// unknown as NOT dormant. Accuracy first: never demote on evidence we don't
/// have. A timestamp in the future (clock skew) clamps to 0 days.
pub fn project_dormant_days(last_activity: &str) -> Option<i64> {
    let trimmed = last_activity.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed_utc = chrono::DateTime::parse_from_rfc3339(trimmed)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S")
                .map(|naive| naive.and_utc())
        })
        .ok()?;
    Some((chrono::Utc::now() - parsed_utc).num_days().max(0))
}

/// True when an inactivity span crosses the dormancy threshold.
pub fn is_dormant_days(days: i64) -> bool {
    days > DORMANT_AFTER_DAYS
}

/// Lockfiles that make a directory an independent project rather than a
/// workspace member. A workspace member shares its workspace root's lockfile
/// and is built as part of it, so it rightly inherits the repository's
/// activity. A directory with its OWN lockfile is installed and shipped on its
/// own, and can die while the repository around it stays busy.
const OWN_LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "poetry.lock",
    "uv.lock",
    "Gemfile.lock",
    "composer.lock",
    "go.sum",
];

/// Most recent on-disk activity marker for a project, as an RFC3339 UTC
/// timestamp: the newest mtime among the git activity files and the
/// project's manifest/lockfiles. `None` when nothing is readable — the
/// caller falls back to the scan timestamp (old behaviour).
///
/// Git markers are the reflog FIRST, `HEAD` as fallback — commits move the
/// reflog but not `HEAD` (see `scanner::git_activity_age_days` for the
/// measurement that established this). The `.git` entry is searched from the
/// project dir upward so workspace members without their own `.git` inherit
/// the repository's activity.
///
/// Exception: an independent project NESTED inside a larger repository (its
/// own lockfile, below the repository root) is judged by its own history — see
/// [`nested_project_activity`]. Otherwise a dead folder inside an active
/// monorepo inherits the monorepo's heartbeat and can never go dormant
/// (2026-09-24: `D:\4DA\paddle-webhook`, unused since Signal moved to Stripe,
/// read as "active today" because the 4DA repository was).
pub fn last_activity_from_fs(project_dir: &Path) -> Option<String> {
    if let Some(root) = find_repo_root_upward(project_dir) {
        if !same_dir(&root, project_dir)
            && has_own_lockfile(project_dir)
            && !consumed_by_enclosing_project(&root, project_dir)
        {
            if let Some(ts) = nested_project_activity(&root, project_dir) {
                return Some(ts);
            }
            // git could not answer (not installed, timed out): fall through to
            // the repository-level evidence. Unknown must never read as dormant.
        }
    }

    let mut newest: Option<SystemTime> = None;
    let mut consider = |t: SystemTime| {
        newest = Some(match newest {
            Some(current) if current >= t => current,
            _ => t,
        });
    };

    if let Some(git_dir) = find_git_dir_upward(project_dir) {
        for marker in ["logs/HEAD", "HEAD"] {
            if let Ok(meta) = std::fs::metadata(git_dir.join(marker)) {
                if let Ok(modified) = meta.modified() {
                    consider(modified);
                }
            }
        }
    }

    for manifest in ACTIVITY_MANIFESTS {
        if let Ok(meta) = std::fs::metadata(project_dir.join(manifest)) {
            if let Ok(modified) = meta.modified() {
                consider(modified);
            }
        }
    }

    newest.map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
}

/// The directory holding the nearest `.git` entry, from `start` upward.
fn find_repo_root_upward(start: &Path) -> Option<std::path::PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

fn has_own_lockfile(dir: &Path) -> bool {
    OWN_LOCKFILES.iter().any(|f| dir.join(f).is_file())
}

/// Manifests that can pull in a sibling directory by path.
const CONSUMER_MANIFESTS: &[&str] = &["Cargo.toml", "package.json", "pyproject.toml", "go.mod"];

/// True when a manifest in any directory between `project_dir` and the
/// repository root names the project by its path from there: a Cargo `path =`
/// dependency or workspace member, an npm `file:`/`link:` dependency, a
/// pyproject path source, a go `replace`. Such a directory is a component of
/// the enclosing project (a stray lockfile of its own does not change that:
/// `src-tauri/fourda-macros`) and lives as long as that project does.
fn consumed_by_enclosing_project(repo_root: &Path, project_dir: &Path) -> bool {
    let (Ok(root), Ok(project)) = (repo_root.canonicalize(), project_dir.canonicalize()) else {
        return false;
    };
    let mut ancestor = project.parent();
    while let Some(dir) = ancestor {
        let Ok(rel) = project.strip_prefix(dir) else {
            break;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let needles = [
            format!("\"{rel}\""),
            format!("\"./{rel}\""),
            format!("file:{rel}"),
            format!("file:./{rel}"),
            format!("link:{rel}"),
            format!("link:./{rel}"),
            format!("=> ./{rel}"),
            format!("=> {rel}"),
        ];
        for manifest in CONSUMER_MANIFESTS {
            if let Ok(text) = std::fs::read_to_string(dir.join(manifest)) {
                if needles.iter().any(|n| text.contains(n.as_str())) {
                    return true;
                }
            }
        }
        if dir == root {
            break;
        }
        ancestor = dir.parent();
    }
    false
}

/// Activity of an independent project nested in a larger repository, judged
/// by its own history: the newer of
/// - the last commit that changed anything under it OTHER than its manifests
///   and lockfiles (dependency bumps, most of them automated, are upkeep, not
///   work on the project), and
/// - the newest mtime among its uncommitted changes, same exclusion, so work in
///   progress never reads as dormant.
///
/// `None` when git cannot answer or neither signal exists; the caller then
/// falls back to the repository-level evidence.
fn nested_project_activity(repo_root: &Path, project_dir: &Path) -> Option<String> {
    let root = repo_root.canonicalize().ok()?;
    let project = project_dir.canonicalize().ok()?;
    let rel = project
        .strip_prefix(&root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    if rel.is_empty() {
        return None;
    }

    let excludes: Vec<String> = ACTIVITY_MANIFESTS
        .iter()
        .map(|m| format!(":(exclude){rel}/{m}"))
        .collect();
    let mut log_args = vec!["log", "-1", "--format=%cI", "--", rel.as_str()];
    log_args.extend(excludes.iter().map(String::as_str));
    let committed = super::git::run_git_with_timeout(&log_args, &root)
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| parse_commit_time(&String::from_utf8_lossy(&o.stdout)));

    let status_args = [
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "-z",
        "-uall",
        "--",
        rel.as_str(),
    ];
    let uncommitted = super::git::run_git_with_timeout(&status_args, &root)
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| newest_uncommitted_change(&root, &o.stdout));

    match (committed, uncommitted) {
        (Some(c), Some(u)) => Some(c.max(u)),
        (c, u) => c.or(u),
    }
    .map(|t| t.to_rfc3339())
}

fn parse_commit_time(stdout: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(stdout.trim())
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc))
}

/// Newest mtime among the files `git status --porcelain=v1 -z` lists, skipping
/// manifests and lockfiles. Entries are `XY path\0`; a rename or copy carries
/// its original path as a second NUL-separated field, skipped here. Bounded so
/// a huge untracked tree cannot stall a scan.
fn newest_uncommitted_change(
    repo_root: &Path,
    porcelain: &[u8],
) -> Option<chrono::DateTime<chrono::Utc>> {
    const MAX_ENTRIES: usize = 500;
    let text = String::from_utf8_lossy(porcelain);
    let mut fields = text.split('\0').filter(|f| !f.is_empty());
    let mut newest: Option<SystemTime> = None;
    let mut seen = 0;
    while let Some(entry) = fields.next() {
        if entry.len() < 4 || seen >= MAX_ENTRIES {
            break;
        }
        seen += 1;
        let (status, path) = entry.split_at(3);
        if status.contains('R') || status.contains('C') {
            fields.next();
        }
        let is_manifest = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| ACTIVITY_MANIFESTS.contains(&n));
        if is_manifest {
            continue;
        }
        if let Ok(modified) = std::fs::metadata(repo_root.join(path)).and_then(|m| m.modified()) {
            newest = Some(newest.map_or(modified, |n| n.max(modified)));
        }
    }
    newest.map(chrono::DateTime::<chrono::Utc>::from)
}

/// Walk from `start` upward to the nearest `.git` entry and resolve it to a
/// real git directory (handles linked worktrees, where `.git` is a file).
fn find_git_dir_upward(start: &Path) -> Option<std::path::PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let git_entry = d.join(".git");
        if git_entry.exists() {
            return resolve_git_dir(&git_entry);
        }
        dir = d.parent();
    }
    None
}

#[cfg(test)]
#[path = "dormancy_nested_tests.rs"]
mod nested_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc3339_days_ago(days: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339()
    }

    #[test]
    fn dormant_days_from_rfc3339() {
        let days = project_dormant_days(&rfc3339_days_ago(120)).expect("parseable");
        // Allow one day of slack for the sub-day remainder of `num_days`.
        assert!((119..=120).contains(&days), "got {days}");
        assert!(is_dormant_days(days));
    }

    #[test]
    fn dormant_days_from_sqlite_datetime() {
        let ts = (chrono::Utc::now() - chrono::Duration::days(200))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let days = project_dormant_days(&ts).expect("parseable");
        assert!((199..=200).contains(&days), "got {days}");
    }

    #[test]
    fn recent_activity_is_not_dormant() {
        let days = project_dormant_days(&rfc3339_days_ago(0)).expect("parseable");
        assert_eq!(days, 0);
        assert!(!is_dormant_days(days));
    }

    /// The threshold is exclusive: exactly DORMANT_AFTER_DAYS is still live.
    #[test]
    fn threshold_boundary_is_exclusive() {
        assert!(!is_dormant_days(DORMANT_AFTER_DAYS));
        assert!(is_dormant_days(DORMANT_AFTER_DAYS + 1));
    }

    /// Unknown must never read as dormant — the callers' contract.
    #[test]
    fn unparseable_and_empty_are_none() {
        assert_eq!(project_dormant_days(""), None);
        assert_eq!(project_dormant_days("   "), None);
        assert_eq!(project_dormant_days("not a date"), None);
        assert_eq!(project_dormant_days("2026-13-45"), None);
    }

    /// Clock skew (a future timestamp) clamps to 0, never negative.
    #[test]
    fn future_timestamp_clamps_to_zero() {
        let future = (chrono::Utc::now() + chrono::Duration::days(3)).to_rfc3339();
        assert_eq!(project_dormant_days(&future), Some(0));
    }

    #[test]
    fn fs_activity_from_manifest_mtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").expect("write");
        let ts = last_activity_from_fs(dir.path()).expect("manifest mtime counts");
        let days = project_dormant_days(&ts).expect("rfc3339 output");
        assert_eq!(days, 0, "a just-written manifest is activity NOW");
    }

    #[test]
    fn fs_activity_includes_git_head() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join(".git")).expect("mk .git");
        std::fs::write(dir.path().join(".git").join("HEAD"), "ref: refs/heads/main")
            .expect("write HEAD");
        let ts = last_activity_from_fs(dir.path()).expect("git HEAD mtime counts");
        assert_eq!(project_dormant_days(&ts), Some(0));
    }

    #[test]
    fn fs_activity_none_when_no_markers() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(last_activity_from_fs(dir.path()), None);
    }
}

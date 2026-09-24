// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Liveness of independent projects nested in a larger repository, against
//! REAL git repositories with back-dated commits. Each test returns early when
//! no `git` binary is available; the CI runners and dev machines have one.

use std::path::Path;
use std::process::Command;

use super::*;

/// Run git in `dir` with author and committer dates `days_ago` in the past.
fn git_at(dir: &Path, days_ago: i64, args: &[&str]) -> bool {
    let date = (chrono::Utc::now() - chrono::Duration::days(days_ago)).to_rfc3339();
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn commit_all(repo: &Path, days_ago: i64, msg: &str) {
    assert!(git_at(repo, days_ago, &["add", "-A"]), "git add");
    assert!(
        git_at(repo, days_ago, &["commit", "-q", "-m", msg]),
        "git commit"
    );
}

/// A repo whose root is busy (a commit today) and which holds `sub/`, an
/// independent package with its own lockfile, last worked on 200 days ago.
/// `None` when git is unavailable.
fn monorepo_with_stale_package() -> Option<tempfile::TempDir> {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path();
    if !git_at(repo, 0, &["init", "-q"]) {
        return None;
    }
    let sub = repo.join("sub");
    std::fs::create_dir_all(sub.join("api")).expect("mkdir");
    std::fs::write(sub.join("package.json"), "{}").expect("write");
    std::fs::write(sub.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").expect("write");
    std::fs::write(sub.join("api").join("handler.ts"), "export {}\n").expect("write");
    commit_all(repo, 200, "the package's last real work");

    std::fs::write(repo.join("main.rs"), "fn main() {}\n").expect("write");
    commit_all(repo, 0, "the rest of the repo is busy");
    Some(tmp)
}

fn days(ts: Option<String>) -> i64 {
    project_dormant_days(&ts.expect("activity found")).expect("rfc3339")
}

/// THE case (2026-09-24, paddle-webhook): the repository is active today, but
/// the nested package has not been touched in 200 days. It must read dormant,
/// not inherit the repository's heartbeat.
#[test]
fn nested_package_is_judged_by_its_own_history() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let d = days(last_activity_from_fs(&tmp.path().join("sub")));
    assert!((199..=200).contains(&d), "got {d} days");
    assert!(is_dormant_days(d));
}

/// Dependency upkeep (a Dependabot-style lockfile and manifest bump) is not
/// work on the project, so it does not revive a dead package.
#[test]
fn lockfile_only_commits_do_not_count_as_activity() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let sub = tmp.path().join("sub");
    std::fs::write(
        sub.join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'\n# bumped\n",
    )
    .expect("write");
    std::fs::write(sub.join("package.json"), "{\"version\":\"1.0.1\"}").expect("write");
    commit_all(tmp.path(), 1, "chore(deps): bump");
    assert!(is_dormant_days(days(last_activity_from_fs(&sub))));
}

/// A real code change under the package makes it active again.
#[test]
fn a_code_commit_under_the_package_is_activity() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let sub = tmp.path().join("sub");
    std::fs::write(sub.join("api").join("handler.ts"), "export const x = 1\n").expect("write");
    commit_all(tmp.path(), 3, "feat: real work");
    let d = days(last_activity_from_fs(&sub));
    assert!((2..=3).contains(&d), "got {d} days");
}

/// Work in progress that has not been committed yet is activity NOW.
#[test]
fn uncommitted_edits_count_as_activity() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let sub = tmp.path().join("sub");
    std::fs::write(sub.join("api").join("handler.ts"), "export const wip = 1\n").expect("write");
    std::fs::write(sub.join("api").join("new_file.ts"), "export {}\n").expect("write");
    assert_eq!(days(last_activity_from_fs(&sub)), 0);
}

/// An uncommitted lockfile change alone (an `install`) is still upkeep.
#[test]
fn an_uncommitted_lockfile_change_is_not_activity() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let sub = tmp.path().join("sub");
    std::fs::write(
        sub.join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'\n# local\n",
    )
    .expect("write");
    assert!(is_dormant_days(days(last_activity_from_fs(&sub))));
}

/// A workspace member (no lockfile of its own) is built as part of the busy
/// repository and keeps inheriting its activity, however long it has gone
/// untouched.
#[test]
fn workspace_members_still_inherit_the_repository() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let member = tmp.path().join("member");
    std::fs::create_dir_all(&member).expect("mkdir");
    std::fs::write(member.join("Cargo.toml"), "[package]\nname = \"m\"\n").expect("write");
    commit_all(tmp.path(), 300, "an old, stable workspace crate");
    // The repository's own activity (the reflog) moved with that commit, so
    // the member reads as active.
    assert_eq!(days(last_activity_from_fs(&member)), 0);
}

/// A component the enclosing project consumes by path keeps inheriting the
/// repository even with a stray lockfile of its own (live 2026-09-24:
/// `src-tauri/fourda-macros` has a Cargo.lock and is a `path =` dependency of
/// src-tauri; it read dormant at 136 days before this rule).
#[test]
fn a_path_dependency_of_an_enclosing_project_is_not_independent() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    let app = tmp.path().join("app");
    let macros = app.join("macros");
    std::fs::create_dir_all(&macros).expect("mkdir");
    std::fs::write(
        app.join("Cargo.toml"),
        "[dependencies]\nmacros = { path = \"macros\" }\n",
    )
    .expect("write");
    std::fs::write(macros.join("Cargo.toml"), "[package]\nname = \"macros\"\n").expect("write");
    std::fs::write(macros.join("Cargo.lock"), "# stray\n").expect("write");
    std::fs::write(macros.join("lib.rs"), "\n").expect("write");
    commit_all(tmp.path(), 200, "macros untouched since");
    std::fs::write(tmp.path().join("main.rs"), "fn main() { }\n").expect("write");
    commit_all(tmp.path(), 0, "busy");

    assert!(consumed_by_enclosing_project(tmp.path(), &macros));
    assert_eq!(days(last_activity_from_fs(&macros)), 0);
    // The unreferenced package in the same repository is still judged alone.
    assert!(!consumed_by_enclosing_project(
        tmp.path(),
        &tmp.path().join("sub")
    ));
}

/// The repository root itself keeps the repository-level evidence.
#[test]
fn the_repository_root_is_unchanged() {
    let Some(tmp) = monorepo_with_stale_package() else {
        return;
    };
    std::fs::write(tmp.path().join("Cargo.lock"), "# root lock\n").expect("write");
    assert_eq!(days(last_activity_from_fs(tmp.path())), 0);
}

/// Diagnostic, run by hand: what liveness reads for real project directories.
///   FOURDA_LIVENESS_PROBE="D:\4DA\site;D:\4DA\relay" \
///     cargo test --lib liveness_probe -- --ignored --nocapture
#[test]
#[ignore = "reads real directories named in FOURDA_LIVENESS_PROBE"]
fn liveness_probe() {
    let paths = std::env::var("FOURDA_LIVENESS_PROBE").unwrap_or_default();
    for p in paths.split(';').filter(|p| !p.trim().is_empty()) {
        let dir = Path::new(p.trim());
        let ts = last_activity_from_fs(dir);
        let d = ts.as_deref().and_then(project_dormant_days);
        let consumed = find_repo_root_upward(dir)
            .is_some_and(|root| consumed_by_enclosing_project(&root, dir));
        println!(
            "{p}: own_lockfile={} consumed={consumed} last_activity={ts:?} days={d:?} dormant={}",
            has_own_lockfile(dir),
            d.is_some_and(is_dormant_days)
        );
    }
}

/// Porcelain parsing: renames carry an extra NUL field, manifests are skipped.
#[test]
fn porcelain_parsing_skips_rename_sources_and_manifests() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("a.ts"), "x").expect("write");
    std::fs::write(tmp.path().join("package.json"), "{}").expect("write");
    let porcelain = b"R  a.ts\0old_name.ts\0 M package.json\0";
    let newest = newest_uncommitted_change(tmp.path(), porcelain).expect("a.ts counts");
    let age = chrono::Utc::now() - newest;
    assert!(age.num_minutes() < 5);
    assert_eq!(
        newest_uncommitted_change(tmp.path(), b" M package.json\0"),
        None,
        "a manifest-only change is not activity"
    );
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the lockfile walk (`ace_commands::dependencies`) — extracted so
//! the production module stays under the Rust file-size ceiling (loaded via
//! #[path], the dependency_health_tests.rs precedent).

use super::*;

/// A repository: `.git/` with a config naming `origin` and a fresh `HEAD`
/// (fresh = recent git activity, so relevance is 1.0).
fn repo(dir: &Path, origin: &str) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(
        dir.join(".git").join("config"),
        format!("[remote \"origin\"]\n\turl = {origin}\n"),
    )
    .unwrap();
    std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
}

fn lockfile(dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, "{}").unwrap();
    path
}

fn age_days(path: &Path, days: u64) {
    let then = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(then)
        .unwrap();
}

/// THE live case (2026-09-04 audit): `navcal/vercel-workflow` — a clone
/// of `vercel/workflow` nested in the user's own repo — fed 1,811
/// `user_dependencies` rows. Its lockfile must not be walked; a nested
/// checkout of the SAME repository and a plain subproject still are.
#[test]
fn walk_skips_nested_foreign_repo_and_keeps_own_checkouts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("navcal");
    repo(&root, "https://github.com/runyourempire/NAVCAL.git");
    lockfile(&root, "package-lock.json");

    let foreign = root.join("vercel-workflow");
    repo(&foreign, "https://github.com/vercel/workflow.git");
    lockfile(&foreign, "package-lock.json");
    lockfile(&foreign.join("packages").join("core"), "Cargo.lock");

    let mirror = root.join("tools").join("mirror");
    repo(&mirror, "git@github.com:runyourempire/NAVCAL.git");
    lockfile(&mirror, "Cargo.lock");

    let member = root.join("crates").join("member");
    lockfile(&member, "Cargo.lock");

    let selected = collect_lockfile_dirs(&[root.clone()]);
    assert!(selected.contains(&root), "own repo root: {selected:?}");
    assert!(
        selected.contains(&mirror),
        "same-remote checkout: {selected:?}"
    );
    assert!(selected.contains(&member), "plain subproject: {selected:?}");
    assert!(
        !selected.iter().any(|d| d.starts_with(&foreign)),
        "nested foreign clone (and everything under it) must be skipped: {selected:?}"
    );
}

/// A project with no `.git` scores from its lockfile mtimes: dormant for
/// 120 days is 0.1 (below the 0.15 floor) — skipped like a stale repo;
/// touched today is 1.0 — kept.
#[test]
fn walk_skips_dormant_no_git_project_and_keeps_a_fresh_one() {
    let tmp = tempfile::tempdir().unwrap();
    let dormant = tmp.path().join("kairos-mvp");
    age_days(&lockfile(&dormant, "package-lock.json"), 120);
    let fresh = tmp.path().join("active-app");
    lockfile(&fresh, "Cargo.lock");

    let selected = collect_lockfile_dirs(&[tmp.path().to_path_buf()]);
    assert!(selected.contains(&fresh), "{selected:?}");
    assert!(
        !selected.contains(&dormant),
        "no-git project idle 120 days must be gated out: {selected:?}"
    );
}

/// Directories with nothing to read are never selected, and the build /
/// cache / agent-infra skip list still prunes descent.
#[test]
fn walk_selects_only_dirs_holding_a_lockfile() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    lockfile(&app, "Cargo.lock");
    lockfile(
        &app.join("node_modules").join("left-pad"),
        "package-lock.json",
    );
    lockfile(
        &app.join(".claude").join("worktrees").join("agent-x"),
        "Cargo.lock",
    );
    std::fs::create_dir_all(app.join("src")).unwrap();

    let selected = collect_lockfile_dirs(&[tmp.path().to_path_buf()]);
    assert_eq!(selected, vec![app]);
}

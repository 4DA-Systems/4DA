// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tests for the hardened git entry point used against scanned repositories.

use super::*;
use std::ffi::OsStr;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A throwaway repository under the OS temp dir.
fn temp_repo(tag: &str) -> Option<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "4da-git-hardening-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).ok()?;
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .ok()?
        .success();
    ok.then_some(root)
}

#[test]
fn scanned_repo_git_puts_hardening_before_any_subcommand() {
    let cmd = scanned_repo_git(Path::new("C:/projects/app"));
    let args: Vec<&OsStr> = cmd.get_args().collect();
    let expected: Vec<&OsStr> = SCANNED_REPO_GIT_ARGS.iter().map(OsStr::new).collect();
    assert_eq!(
        args, expected,
        "global options come first, so they apply to every subcommand"
    );
    for (key, value) in [("core.fsmonitor", "false"), ("log.showSignature", "false")] {
        let pair = format!("{key}={value}");
        let at = SCANNED_REPO_GIT_ARGS
            .iter()
            .position(|a| *a == pair)
            .unwrap_or_else(|| panic!("{pair} must be set"));
        assert_eq!(SCANNED_REPO_GIT_ARGS[at - 1], "-c");
    }
    assert!(SCANNED_REPO_GIT_ARGS.contains(&"--no-pager"));
}

#[test]
fn scanned_repo_git_never_prompts_for_credentials() {
    let cmd = scanned_repo_git(Path::new("C:/projects/app"));
    let prompt = cmd
        .get_envs()
        .find(|(k, _)| *k == OsStr::new("GIT_TERMINAL_PROMPT"))
        .and_then(|(_, v)| v);
    assert_eq!(prompt, Some(OsStr::new("0")));
    assert_eq!(cmd.get_current_dir(), Some(Path::new("C:/projects/app")));
}

#[test]
fn only_repository_scoped_filters_count_as_untrusted() {
    // The user's own global filters (Git LFS is the common one) are trusted.
    assert!(!filter_lines_include_repo_scope(
        "global\tfilter.lfs.clean git-lfs clean -- %f\n\
         system\tfilter.lfs.process git-lfs filter-process\n"
    ));
    assert!(filter_lines_include_repo_scope(
        "global\tfilter.lfs.clean git-lfs clean -- %f\nlocal\tfilter.x.clean cat\n"
    ));
    assert!(filter_lines_include_repo_scope(
        "worktree\tfilter.x.smudge cat\n"
    ));
    assert!(!filter_lines_include_repo_scope(""));
}

#[test]
fn a_plain_repository_defines_no_content_filters() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let Some(repo) = temp_repo("plain") else {
        eprintln!("skipped: could not create a temp repository");
        return;
    };
    let defined = repo_defines_content_filters(&repo);
    let _ = std::fs::remove_dir_all(&repo);
    assert!(
        !defined,
        "a fresh repository has no repository-scoped filter"
    );
}

#[test]
fn a_repository_scoped_filter_is_detected_so_status_is_skipped() {
    if !git_available() {
        eprintln!("skipped: git not available");
        return;
    }
    let Some(repo) = temp_repo("filter") else {
        eprintln!("skipped: could not create a temp repository");
        return;
    };
    // `git config` only records the value; nothing runs it here.
    let recorded = Command::new("git")
        .args(["config", "filter.sample.clean", "cat"])
        .current_dir(&repo)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let defined = repo_defines_content_filters(&repo);
    let _ = std::fs::remove_dir_all(&repo);
    assert!(recorded, "test setup must record the filter");
    assert!(defined, "a repository-scoped filter must be detected");
}

#[test]
fn an_unreadable_location_fails_closed() {
    // Not a repository and not even a directory: git cannot answer, so the
    // caller must treat it as defining a filter and skip `status`.
    let missing = std::env::temp_dir().join("4da-git-hardening-does-not-exist-xyz");
    assert!(repo_defines_content_filters(&missing));
}

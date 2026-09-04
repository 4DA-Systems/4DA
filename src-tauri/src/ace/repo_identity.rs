// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Repository identity for directory walks — is this nested `.git` the user's
//! project, or a third-party clone sitting inside it?
//!
//! Live evidence (2026-09-04 audit): `Documents/navcal/vercel-workflow` is a
//! clone of `github.com/vercel/workflow` nested inside the user's own `navcal`
//! repo (`runyourempire/NAVCAL`). Nothing in the lockfile walk asked whose
//! repository a directory belonged to, so the clone contributed 1,811 of 7,933
//! `user_dependencies` rows and every `rkyv` advisory on the Preemption
//! surface. A nested checkout whose `remote "origin"` differs from the
//! enclosing repository's is somebody else's code; its lockfiles describe
//! their stack, not the user's.

use std::path::{Path, PathBuf};

use super::scanner::resolve_git_dir;

/// Where a directory walk currently stands relative to git repositories.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RepoScope {
    /// No `.git` at or above the current directory.
    Outside,
    /// Inside a repository whose `remote "origin"` URL is `remote`
    /// (`None` when the repository has no origin configured).
    Inside { remote: Option<String> },
}

/// Outcome of stepping a walk into `dir`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Keep walking; `dir` lives in this scope.
    Continue(RepoScope),
    /// `dir` is a checkout of a DIFFERENT repository than the one enclosing
    /// it. Both remotes are carried so the skip can be logged with evidence.
    ForeignRepo {
        nested: Option<String>,
        enclosing: Option<String>,
    },
}

/// Scope of a walk ROOT: the nearest repository at or above `dir`, so a scan
/// rooted inside a repo (`D:\4DA\src-tauri`) still knows whose repo it is in.
pub(crate) fn scope_at(dir: &Path) -> RepoScope {
    let mut cursor = Some(dir);
    while let Some(d) = cursor {
        if d.join(".git").exists() {
            return RepoScope::Inside {
                remote: origin_remote(d),
            };
        }
        cursor = d.parent();
    }
    RepoScope::Outside
}

/// Step into `dir` from `scope`. A directory without its own `.git` inherits
/// the scope; one with its own `.git` becomes the new scope unless it is a
/// foreign checkout nested inside another repository.
pub(crate) fn step_into(dir: &Path, scope: &RepoScope) -> Step {
    if !dir.join(".git").exists() {
        return Step::Continue(scope.clone());
    }
    let nested = origin_remote(dir);
    match scope {
        RepoScope::Outside => Step::Continue(RepoScope::Inside { remote: nested }),
        RepoScope::Inside { remote: enclosing } => {
            if same_remote(nested.as_deref(), enclosing.as_deref()) {
                Step::Continue(RepoScope::Inside { remote: nested })
            } else {
                Step::ForeignRepo {
                    nested,
                    enclosing: enclosing.clone(),
                }
            }
        }
    }
}

/// Two origin URLs name the same repository. Two repositories with NO origin
/// at all cannot be told apart, so they are treated as the same project
/// (accuracy first: never drop a user's own code on absent evidence); a
/// remote on exactly one side is a different repository.
pub(crate) fn same_remote(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => normalize_remote(a) == normalize_remote(b),
        _ => false,
    }
}

/// `remote "origin"` URL of the repository whose `.git` entry sits directly
/// in `dir` (`None` when there is no `.git`, no config, or no origin).
pub(crate) fn origin_remote(dir: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(&dir.join(".git"))?;
    let config = std::fs::read_to_string(git_config_path(&git_dir)).ok()?;
    parse_origin_url(&config)
}

/// A linked worktree's gitdir (`.git/worktrees/<name>`) has no `config` of its
/// own; `commondir` points at the shared `.git` that holds the remotes.
fn git_config_path(git_dir: &Path) -> PathBuf {
    if let Ok(common) = std::fs::read_to_string(git_dir.join("commondir")) {
        let common = common.trim();
        if !common.is_empty() {
            let common_dir = if Path::new(common).is_absolute() {
                PathBuf::from(common)
            } else {
                git_dir.join(common)
            };
            return common_dir.join("config");
        }
    }
    git_dir.join("config")
}

/// Extract `url` from the `[remote "origin"]` section of a git config file.
pub(crate) fn parse_origin_url(config: &str) -> Option<String> {
    let mut in_origin = false;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_origin = is_origin_header(trimmed);
            continue;
        }
        if !in_origin {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            if key.trim() == "url" && !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn is_origin_header(header: &str) -> bool {
    let inner = header.trim_start_matches('[').trim_end_matches(']').trim();
    inner
        .strip_prefix("remote")
        .is_some_and(|rest| rest.trim().trim_matches('"') == "origin")
}

/// Comparison form of a remote URL: the same repository reached over HTTPS,
/// SSH, or scp-style syntax, with or without `.git`, must compare equal.
pub(crate) fn normalize_remote(url: &str) -> String {
    let lower = url.trim().to_lowercase();
    let no_scheme = lower
        .split_once("://")
        .map_or(lower.as_str(), |(_, rest)| rest);
    let no_user = match no_scheme.split_once('@') {
        Some((user, rest)) if !user.contains('/') => rest,
        _ => no_scheme,
    };
    // scp-like `host:owner/repo` -> `host/owner/repo` (colon before any slash).
    let path_form = match (no_user.find(':'), no_user.find('/')) {
        (Some(colon), Some(slash)) if colon < slash => no_user.replacen(':', "/", 1),
        (Some(_), None) => no_user.replacen(':', "/", 1),
        _ => no_user.to_string(),
    };
    path_form
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_repo(dir: &Path, origin: Option<&str>) {
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let config = match origin {
            Some(url) => format!(
                "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = {url}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n"
            ),
            None => "[core]\n\tbare = false\n".to_string(),
        };
        std::fs::write(dir.join(".git").join("config"), config).unwrap();
    }

    #[test]
    fn https_ssh_and_scp_forms_of_one_repo_compare_equal() {
        let forms = [
            "https://github.com/vercel/workflow.git",
            "https://github.com/Vercel/Workflow",
            "git@github.com:vercel/workflow.git",
            "ssh://git@github.com/vercel/workflow.git",
            "https://user@github.com/vercel/workflow/",
        ];
        for f in forms {
            assert_eq!(normalize_remote(f), "github.com/vercel/workflow", "{f}");
        }
        assert_ne!(
            normalize_remote("https://github.com/runyourempire/NAVCAL.git"),
            normalize_remote("https://github.com/vercel/workflow.git")
        );
    }

    #[test]
    fn origin_url_is_read_from_the_origin_section_only() {
        let config = "[core]\n\turl = not-this\n[remote \"upstream\"]\n\turl = https://x/upstream.git\n[remote \"origin\"]\n\turl = https://x/origin.git\n[branch \"main\"]\n\tremote = origin\n";
        assert_eq!(
            parse_origin_url(config).as_deref(),
            Some("https://x/origin.git")
        );
        assert_eq!(parse_origin_url("[core]\n\tbare = false\n"), None);
    }

    /// THE live case: a third-party clone nested inside the user's own repo.
    #[test]
    fn nested_clone_with_a_different_origin_is_foreign() {
        let tmp = tempfile::tempdir().unwrap();
        let navcal = tmp.path().join("navcal");
        write_repo(&navcal, Some("https://github.com/runyourempire/NAVCAL.git"));
        let vendored = navcal.join("vercel-workflow");
        write_repo(&vendored, Some("https://github.com/vercel/workflow.git"));

        let own = RepoScope::Inside {
            remote: Some("https://github.com/runyourempire/NAVCAL.git".into()),
        };
        assert_eq!(
            step_into(&navcal, &RepoScope::Outside),
            Step::Continue(own.clone()),
            "the user's own repo must be entered"
        );
        assert_eq!(
            step_into(&vendored, &own),
            Step::ForeignRepo {
                nested: Some("https://github.com/vercel/workflow.git".into()),
                enclosing: Some("https://github.com/runyourempire/NAVCAL.git".into()),
            },
            "a nested clone of somebody else's repo must be skipped"
        );
    }

    /// A nested checkout of the SAME repository (a worktree parked inside the
    /// tree, an ssh-form clone of the https-form parent) is still the user's.
    #[test]
    fn nested_checkout_of_the_same_repo_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("app");
        write_repo(&root, Some("https://github.com/me/app.git"));
        let nested = root.join("scratch").join("app-copy");
        write_repo(&nested, Some("git@github.com:me/app.git"));

        let scope = RepoScope::Inside {
            remote: origin_remote(&root),
        };
        assert!(matches!(
            step_into(&nested, &scope),
            Step::Continue(RepoScope::Inside { .. })
        ));
        // A plain subdirectory inherits the scope unchanged.
        let plain = root.join("src");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(step_into(&plain, &scope), Step::Continue(scope.clone()));
    }

    #[test]
    fn a_nested_repo_with_a_remote_inside_a_remoteless_repo_is_foreign() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("local-only");
        write_repo(&root, None);
        let nested = root.join("vendor-clone");
        write_repo(&nested, Some("https://github.com/other/thing.git"));
        let scope = RepoScope::Inside { remote: None };
        assert!(matches!(
            step_into(&nested, &scope),
            Step::ForeignRepo { .. }
        ));
        // Two remoteless repos cannot be told apart — kept.
        assert!(same_remote(None, None));
    }

    #[test]
    fn scope_at_finds_the_enclosing_repo_above_a_scan_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        write_repo(&root, Some("https://github.com/me/repo.git"));
        let inner = root.join("crates").join("member");
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(
            scope_at(&inner),
            RepoScope::Inside {
                remote: Some("https://github.com/me/repo.git".into())
            }
        );
        let outside = tmp.path().join("no-repo-here");
        std::fs::create_dir_all(&outside).unwrap();
        // tempdir itself is not a repo; nothing above it in the test tree is
        // guaranteed either, so only assert when the walk finds nothing.
        if scope_at(tmp.path()) == RepoScope::Outside {
            assert_eq!(scope_at(&outside), RepoScope::Outside);
        }
    }

    /// Linked worktrees keep their remotes in the common `.git`, reached via
    /// `commondir` from the per-worktree gitdir.
    #[test]
    fn linked_worktree_reads_remotes_from_the_common_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        write_repo(&main, Some("https://github.com/me/app.git"));
        let wt_gitdir = main.join(".git").join("worktrees").join("wt");
        std::fs::create_dir_all(&wt_gitdir).unwrap();
        std::fs::write(wt_gitdir.join("commondir"), "../..").unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}", wt_gitdir.display())).unwrap();
        assert_eq!(
            origin_remote(&wt).as_deref(),
            Some("https://github.com/me/app.git")
        );
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Is this project directory a scratch tree its own repository ignores?
//!
//! The scanner's exclusion tiers ([`crate::project_inclusion`]) are hardcoded
//! name lists — `node_modules`, `target`, `fixtures`, `.claude`. They have no
//! idea what the user's own `.gitignore` says, so a scratch tree with a
//! plausible name is a first-class project on every surface.
//!
//! Live 2026-09-07: `D:\4DA\victauri-gauntlet` is gitignored by 4DA's own
//! `.gitignore` and carries its own `Cargo.lock`. Its `anyhow` and `openssl`
//! advisories were presented beside the user's real findings with nothing to
//! tell them apart — a throwaway harness reading as the user's security
//! posture.
//!
//! `git check-ignore` is the only correct oracle here: `.gitignore` files
//! nest, negate (`!pattern`), and compose with `.git/info/exclude` and the
//! global excludes file. Reimplementing that would be a second, wrong
//! implementation of a spec git already ships.
//!
//! # This LABELS, it never suppresses
//!
//! A gitignored project is still a real project with real dependencies the
//! user may still deploy. Nothing here changes urgency or drops a finding —
//! the flag exists so a surface can say "(scratch, gitignored)" and let the
//! user tell a gauntlet from a product. Unreachable *findings* are a separate
//! problem, solved by [`crate::ace::cargo_resolve`].
//!
//! # Conservative on every failure
//!
//! No git, not a repository, an errored probe, a timeout: NOT scratch. The
//! only `true` comes from git exiting 0 on `check-ignore -q`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

/// `git check-ignore` is a single stat-bound query; anything slower than this
/// is a hung child, and a hung child means "not scratch".
const CHECK_IGNORE_TIMEOUT: Duration = Duration::from_secs(10);

/// Per-scan memo. One scan revisits the same directories through the manifest
/// walk and the lockfile walk, and each probe is a process spawn.
#[derive(Default)]
pub(crate) struct ScratchProbe {
    cache: HashMap<String, bool>,
}

impl ScratchProbe {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Is `dir` ignored by the git repository that encloses it?
    pub(crate) fn is_scratch(&mut self, dir: &Path) -> bool {
        let key = dir.to_string_lossy().to_lowercase();
        if let Some(hit) = self.cache.get(&key) {
            return *hit;
        }
        let verdict = git_ignores(dir);
        self.cache.insert(key, verdict);
        verdict
    }
}

/// True only when `git check-ignore -q` exits 0 for `dir`.
///
/// Exit codes are the contract: 0 = ignored, 1 = not ignored, 128 = not a
/// repository (or another fatal). Only 0 is a `true`.
fn git_ignores(dir: &Path) -> bool {
    let Some(name) = dir.file_name() else {
        // A filesystem root has no path for git to test.
        return false;
    };
    let Some(parent) = dir.parent() else {
        return false;
    };
    let mut cmd = std::process::Command::new("git");
    // Run from the PARENT and name the child: `check-ignore` on the repo's
    // own root would ask whether the repository ignores itself.
    cmd.arg("check-ignore")
        .arg("-q")
        .arg("--")
        .arg(name)
        .current_dir(parent)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let Ok(mut child) = cmd.spawn() else {
        return false; // git absent -> nothing is scratch
    };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if start.elapsed() > CHECK_IGNORE_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return false,
        }
    }
}

/// The suffix a surface appends when naming a scratch project.
pub(crate) fn scratch_label() -> &'static str {
    "(scratch, gitignored)"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a real git repo whose `.gitignore` hides one of two sibling
    /// project dirs. Returns the repo root, or `None` when git is unusable
    /// here — the assertions are then skipped rather than failing on an
    /// environment condition.
    fn repo_with_ignored_dir() -> Option<std::path::PathBuf> {
        let root = std::env::temp_dir().join(format!(
            "4da-scratch-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("real-app")).ok()?;
        std::fs::create_dir_all(root.join("gauntlet")).ok()?;
        std::fs::write(root.join(".gitignore"), "gauntlet/\n").ok()?;
        let mut cmd = std::process::Command::new("git");
        cmd.args(["init", "-q"])
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Stated, not inferred: the no-window gate accepts this site by a
        // helper-name heuristic, and a test run should not flash a console
        // window on Windows either.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
        ok.then_some(root)
    }

    #[test]
    fn gitignored_directory_is_scratch_and_its_sibling_is_not() {
        let Some(root) = repo_with_ignored_dir() else {
            return; // no usable git here
        };
        let mut probe = ScratchProbe::new();
        assert!(
            probe.is_scratch(&root.join("gauntlet")),
            "a directory the repo gitignores is scratch"
        );
        // The negative half of the gate: a normal project must NEVER be
        // labelled, or the label means nothing.
        assert!(
            !probe.is_scratch(&root.join("real-app")),
            "a tracked sibling directory is not scratch"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_directory_outside_any_repository_is_not_scratch() {
        let dir = std::env::temp_dir().join("4da-scratch-no-repo");
        std::fs::create_dir_all(&dir).ok();
        let mut probe = ScratchProbe::new();
        assert!(
            !probe.is_scratch(&dir),
            "no repository means no verdict, and no verdict means not scratch"
        );
    }

    #[test]
    fn a_nonexistent_path_is_not_scratch() {
        let mut probe = ScratchProbe::new();
        assert!(!probe.is_scratch(Path::new("Z:/no/such/place/at/all")));
    }

    #[test]
    fn repeated_probes_are_memoized() {
        let mut probe = ScratchProbe::new();
        let dir = std::env::temp_dir().join("4da-scratch-memo");
        std::fs::create_dir_all(&dir).ok();
        let first = probe.is_scratch(&dir);
        assert_eq!(probe.cache.len(), 1, "one probe cached");
        assert_eq!(probe.is_scratch(&dir), first, "second lookup is the memo");
        assert_eq!(probe.cache.len(), 1, "and spawned nothing new");
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Lockfile readers: every installed `(ecosystem, name, version, scope)` a
//! directory's lockfiles name, plus an account of every lockfile that could
//! NOT be read.
//!
//! The readers live in the pure engine crate (`fourda_depengine::lockfile`,
//! single-engine C0.3) and see files only through a host `FileSource`. This
//! module is the host: [`StdFs`] gives them `std::fs`, and the directory
//! walk (which directories to read: [`SKIPPED_DIR_NAMES`],
//! [`MAX_WALK_DEPTH`]) and the walk's report stay here. The types are
//! re-exported under their old paths, so no caller changed.
//!
//! API for conformance harnesses (no database, no network):
//! - [`read_dir`] — the outcomes for ONE directory, exactly what the lockfile
//!   walk stores.
//! - [`parse_lockfiles_under`] — every directory under a root (the walk's
//!   skip list and depth, without the user-scope gates), flattened to
//!   [`FlatDependency`] rows.

use std::path::{Path, PathBuf};

use fourda_depengine::lockfile::{self as engine, Entry, FileSource};

pub(crate) mod report;

pub(crate) use fourda_depengine::lockfile::{
    normalize_descriptor, DepScope, LockFormat, LockedPackage, LockfileOutcome, LockfileRead,
};
pub(crate) use report::LockfileReport;

/// The app's [`FileSource`]: the real file system, paths spelled the way
/// `std::path` joins them on this platform (`D:\app\package-lock.json`).
pub(crate) struct StdFs;

impl FileSource for StdFs {
    fn read(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn is_file(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn list(&self, dir: &str) -> Option<Vec<Entry>> {
        let entries = std::fs::read_dir(dir).ok()?;
        Some(
            entries
                .flatten()
                .map(|e| Entry {
                    name: e.file_name().to_string_lossy().into_owned(),
                    is_file: e.path().is_file(),
                })
                .collect(),
        )
    }

    fn join(&self, dir: &str, name: &str) -> String {
        Path::new(dir).join(name).to_string_lossy().into_owned()
    }

    fn parent(&self, path: &str) -> Option<String> {
        Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
    }
}

fn host(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The first dependency file in `dir` (readable or not), for the walk's
/// relevance probe. `None` when the directory holds none.
pub(crate) fn probe(dir: &Path) -> Option<PathBuf> {
    engine::probe(&StdFs, &host(dir)).map(PathBuf::from)
}

/// Read every dependency file in `dir`. Never fails: a file that cannot be
/// read becomes a [`LockfileOutcome::Failed`].
pub(crate) fn read_dir(dir: &Path) -> Vec<LockfileOutcome> {
    engine::read_dir(&StdFs, &host(dir))
}

/// Parse lockfile text as `format`. `path` names the file in the outcome; the
/// Go and requirements readers also read companions (`go.sum`, `-r`
/// includes) beside it.
#[cfg(test)]
pub(crate) fn read_text(path: &Path, format: LockFormat, content: &str) -> LockfileOutcome {
    engine::read_text(&StdFs, &host(path), format, content)
}

/// The requirements files a directory holds (`*requirements*.txt`,
/// `requirements/*.txt`), sorted.
#[cfg(test)]
pub(crate) fn requirements_files(dir: &Path) -> Vec<PathBuf> {
    engine::requirements_files(&StdFs, &host(dir))
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// One row of [`parse_lockfiles_under`].
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlatDependency {
    /// OSV ecosystem name (`npm`, `crates.io`, `PyPI`, `Go`, `RubyGems`, `Packagist`).
    pub ecosystem: &'static str,
    pub name: String,
    pub version: String,
    /// `runtime` | `dev` | `unknown`.
    pub scope: &'static str,
    /// The directory the walk attributes the package to.
    pub project_dir: PathBuf,
    pub lockfile: PathBuf,
}

/// Every lockfile under `root` read as the walk reads it — same skip list
/// (`node_modules`, `target`, `.git`, …) and depth — WITHOUT the user-scope
/// gates (relevance, foreign checkouts, exclusions), which are policy, not
/// parsing. Returns the flattened packages and every non-read outcome.
#[cfg(test)]
pub(crate) fn parse_lockfiles_under(root: &Path) -> (Vec<FlatDependency>, Vec<LockfileOutcome>) {
    let mut packages = Vec::new();
    let mut problems = Vec::new();
    for dir in walk_dirs(root) {
        for outcome in read_dir(&dir) {
            match outcome {
                LockfileOutcome::Read(read) => {
                    let ecosystem = crate::ecosystem::Ecosystem::parse(read.format.ecosystem())
                        .map_or("unknown", |e| e.osv_name());
                    packages.extend(read.packages.iter().map(|p| FlatDependency {
                        ecosystem,
                        name: p.name.clone(),
                        version: p.version.clone(),
                        scope: p.scope.as_str(),
                        project_dir: dir.clone(),
                        lockfile: PathBuf::from(&read.path),
                    }));
                }
                other => problems.push(other),
            }
        }
    }
    (packages, problems)
}

/// Directory names a lockfile walk never descends into: build output,
/// package caches and agent infrastructure.
pub(crate) const SKIPPED_DIR_NAMES: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    ".next",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    ".cargo",
    ".claude",
    ".codex",
];

/// The walk's maximum depth below a root.
pub(crate) const MAX_WALK_DEPTH: u8 = 5;

/// Every directory under `root` (depth <= [`MAX_WALK_DEPTH`], skip list
/// applied) holding at least one dependency file, sorted.
#[cfg(test)]
pub(crate) fn walk_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_WALK_DEPTH {
            continue;
        }
        if probe(&dir).is_some() {
            out.push(dir.clone());
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let skipped = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_none_or(|n| SKIPPED_DIR_NAMES.contains(&n));
            if path.is_dir() && !skipped {
                stack.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod tests;

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Lockfile readers: every installed `(ecosystem, name, version, scope)` a
//! directory's lockfiles name, plus an account of every lockfile that could
//! NOT be read.
//!
//! Measured 2026-10-10 on 19 public repositories against osv-scanner,
//! cargo-audit, pip-audit and govulncheck, the per-format parsers that lived
//! in `scanner.rs` lost installed packages before OSV was ever asked: nested
//! `node_modules` copies were dropped, yarn berry's `version:` line was not
//! recognised, bun.lock / uv.lock / Pipfile.lock and `requirements-*.txt`
//! had no reader at all, and Go read every version go.sum has ever hashed as
//! installed (precision 7.9%). Each of those was silent. This module reads
//! them and reports what it could not ([`LockfileOutcome`]).
//!
//! API for conformance harnesses (no database, no network):
//! - [`read_dir`] — the outcomes for ONE directory, exactly what the lockfile
//!   walk stores.
//! - [`parse_lockfiles_under`] — every directory under a root (the walk's
//!   skip list and depth, without the user-scope gates), flattened to
//!   [`FlatDependency`] rows.

use std::path::{Path, PathBuf};

use crate::ace::scanner::DependencyEdge;

mod bun;
mod go;
mod npm;
mod python;
pub(crate) mod report;
mod ruby;
mod yarn;

pub(crate) use python::requirements_files;
pub(crate) use report::LockfileReport;

/// A lockfile format this module reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LockFormat {
    NpmLock,
    NpmShrinkwrap,
    Pnpm,
    YarnClassic,
    YarnBerry,
    Bun,
    Cargo,
    Poetry,
    Uv,
    Pdm,
    Pipfile,
    Requirements,
    GoMod,
    Gemfile,
    Composer,
}

impl LockFormat {
    /// The ACE language string the dependency tables key this format by.
    pub(crate) fn ecosystem(self) -> &'static str {
        match self {
            Self::NpmLock
            | Self::NpmShrinkwrap
            | Self::Pnpm
            | Self::YarnClassic
            | Self::YarnBerry
            | Self::Bun => "javascript",
            Self::Cargo => "rust",
            Self::Poetry | Self::Uv | Self::Pdm | Self::Pipfile | Self::Requirements => "python",
            Self::GoMod => "go",
            Self::Gemfile => "ruby",
            Self::Composer => "php",
        }
    }
}

/// Where an installed copy ships, as far as its lockfile says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DepScope {
    /// The lockfile records nothing about dev vs runtime.
    Unknown,
    /// Reached only from dev roots.
    Dev,
    /// Reached from a runtime root.
    Runtime,
}

impl DepScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Dev => "dev",
            Self::Runtime => "runtime",
        }
    }

    /// Two lockfiles (or two paths in one) that disagree: runtime wins over
    /// dev, and any recorded scope over unknown.
    pub(crate) fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

/// One installed copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LockedPackage {
    pub name: String,
    pub version: String,
    pub scope: DepScope,
    /// The copy the project itself resolves the name to (the hoisted npm
    /// copy, the go.mod requirement). The collapsed per-name row keeps it.
    pub primary: bool,
}

impl LockedPackage {
    pub(crate) fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            scope: DepScope::Unknown,
            primary: false,
        }
    }

    pub(crate) fn scoped(mut self, scope: DepScope) -> Self {
        self.scope = scope;
        self
    }

    pub(crate) fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }
}

/// A lockfile that was read.
#[derive(Debug, Clone)]
pub(crate) struct LockfileRead {
    pub path: PathBuf,
    pub format: LockFormat,
    pub packages: Vec<LockedPackage>,
    /// Parent -> child graph, where the format records one and a parser
    /// exists (package-lock, pnpm).
    pub edges: Vec<DependencyEdge>,
    /// Entries that name no registry release (workspace members, links,
    /// git / file / path sources). Not advisory-matchable, but counted so a
    /// lockfile that yields nothing says why.
    pub non_registry_entries: usize,
}

/// What happened to one dependency file in a directory.
#[derive(Debug, Clone)]
pub(crate) enum LockfileOutcome {
    Read(LockfileRead),
    /// Present, but unreadable or unparseable.
    Failed {
        path: PathBuf,
        format: Option<LockFormat>,
        reason: String,
    },
    /// A dependency file of an ecosystem or format 4DA does not read.
    Unsupported {
        path: PathBuf,
        kind: &'static str,
    },
}

/// Fixed-name lockfiles and their formats. yarn.lock is classified by
/// content (classic vs berry) when read.
const READERS: &[(&str, LockFormat)] = &[
    ("package-lock.json", LockFormat::NpmLock),
    ("npm-shrinkwrap.json", LockFormat::NpmShrinkwrap),
    ("pnpm-lock.yaml", LockFormat::Pnpm),
    ("yarn.lock", LockFormat::YarnClassic),
    ("bun.lock", LockFormat::Bun),
    ("Cargo.lock", LockFormat::Cargo),
    ("poetry.lock", LockFormat::Poetry),
    ("uv.lock", LockFormat::Uv),
    ("pdm.lock", LockFormat::Pdm),
    ("Pipfile.lock", LockFormat::Pipfile),
    ("go.mod", LockFormat::GoMod),
    ("Gemfile.lock", LockFormat::Gemfile),
    ("composer.lock", LockFormat::Composer),
];

/// Dependency files 4DA sees but does not read for installed versions.
/// Reported, never silently passed over.
const UNSUPPORTED: &[(&str, &str)] = &[
    (
        "bun.lockb",
        "bun binary lockfile (bun >= 1.2 writes the readable bun.lock)",
    ),
    ("pom.xml", "Maven (installed versions not resolved)"),
    ("build.gradle", "Gradle (installed versions not resolved)"),
    (
        "build.gradle.kts",
        "Gradle (installed versions not resolved)",
    ),
    ("gradle.lockfile", "Gradle lockfile"),
    ("packages.lock.json", "NuGet lockfile"),
    ("pubspec.lock", "Dart pub lockfile"),
    ("Podfile.lock", "CocoaPods lockfile"),
    ("mix.lock", "Elixir mix lockfile"),
    ("Package.resolved", "SwiftPM lockfile"),
    ("conda-lock.yml", "conda lockfile"),
];

/// The first dependency file in `dir` (readable or not), for the walk's
/// relevance probe. `None` when the directory holds none.
pub(crate) fn probe(dir: &Path) -> Option<PathBuf> {
    READERS
        .iter()
        .map(|(name, _)| *name)
        .chain(UNSUPPORTED.iter().map(|(name, _)| *name))
        .chain(std::iter::once("go.sum"))
        .map(|name| dir.join(name))
        .find(|p| p.is_file())
        .or_else(|| requirements_files(dir).into_iter().next())
}

/// Read every dependency file in `dir`. Never fails: a file that cannot be
/// read becomes a [`LockfileOutcome::Failed`].
pub(crate) fn read_dir(dir: &Path) -> Vec<LockfileOutcome> {
    let mut out = Vec::new();
    for (name, format) in READERS {
        let path = dir.join(name);
        if path.is_file() {
            out.push(read_file(&path, *format));
        }
    }
    let go_sum = dir.join("go.sum");
    if go_sum.is_file() && !dir.join("go.mod").is_file() {
        out.push(LockfileOutcome::Failed {
            path: go_sum,
            format: None,
            reason: "go.sum without go.mod: the build list cannot be resolved".to_string(),
        });
    }
    let requirements = requirements_files(dir);
    if !requirements.is_empty() {
        out.extend(python::read_requirements_set(&requirements));
    }
    for (name, kind) in UNSUPPORTED {
        let path = dir.join(name);
        if path.is_file() {
            out.push(LockfileOutcome::Unsupported { path, kind });
        }
    }
    out
}

fn read_file(path: &Path, format: LockFormat) -> LockfileOutcome {
    match std::fs::read_to_string(path) {
        Ok(content) => read_text(path, format, &content),
        Err(e) => LockfileOutcome::Failed {
            path: path.to_path_buf(),
            format: Some(format),
            reason: format!("unreadable: {e}"),
        },
    }
}

/// Parse lockfile text as `format`. `path` names the file in the outcome; the
/// Go and requirements readers also resolve siblings (`go.sum`, `-r`
/// includes) against it.
pub(crate) fn read_text(path: &Path, format: LockFormat, content: &str) -> LockfileOutcome {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let parsed = match format {
        LockFormat::NpmLock | LockFormat::NpmShrinkwrap => npm::read_package_lock(content, format),
        LockFormat::Pnpm => Ok(npm::read_pnpm_lock(content)),
        LockFormat::YarnClassic | LockFormat::YarnBerry => yarn::read_yarn_lock(content),
        LockFormat::Bun => bun::read_bun_lock(content),
        LockFormat::Cargo => Ok(read_cargo_lock(content)),
        LockFormat::Poetry | LockFormat::Uv | LockFormat::Pdm => {
            Ok(python::read_toml_package_lock(content, format))
        }
        LockFormat::Pipfile => python::read_pipfile_lock(content),
        LockFormat::Requirements => Ok(python::read_requirements(path, content)),
        LockFormat::GoMod => Ok(go::read_go_module(path, content)),
        LockFormat::Gemfile => Ok(ruby::read_gemfile_lock(content)),
        LockFormat::Composer => serde_json::from_str::<serde_json::Value>(content)
            .map(|_| {
                plain(
                    format,
                    crate::ace::scanner::ProjectScanner::parse_composer_lock(content),
                )
            })
            .map_err(|e| format!("invalid JSON: {e}")),
    };
    match parsed {
        Ok(mut read) => {
            read.path = path.to_path_buf();
            LockfileOutcome::Read(read)
        }
        Err(reason) => LockfileOutcome::Failed {
            path: path.to_path_buf(),
            format: Some(format),
            reason,
        },
    }
}

/// A read whose packages carry no scope and no graph.
pub(crate) fn plain(format: LockFormat, packages: Vec<(String, String)>) -> LockfileRead {
    let mut seen = std::collections::HashSet::new();
    LockfileRead {
        path: PathBuf::new(),
        format,
        packages: packages
            .into_iter()
            .map(|(name, version)| {
                let first = seen.insert(name.to_lowercase());
                LockedPackage::new(name, version).primary(first)
            })
            .collect(),
        edges: Vec::new(),
        non_registry_entries: 0,
    }
}

/// Cargo.lock minus the project's own crates (workspace members and path
/// dependencies carry no `source`). Used by the conformance API; the store
/// path keeps its own Cargo processor (`ace_commands::dependencies`).
fn read_cargo_lock(content: &str) -> LockfileRead {
    let local = crate::ace::cargo_lock_facts::local_packages(content);
    let all = crate::ace::scanner::ProjectScanner::parse_cargo_lock(content);
    let total = all.len();
    let packages: Vec<(String, String)> = all.into_iter().filter(|k| !local.contains(k)).collect();
    let mut read = plain(LockFormat::Cargo, packages);
    read.non_registry_entries = total - read.packages.len();
    read
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
                        lockfile: read.path.clone(),
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

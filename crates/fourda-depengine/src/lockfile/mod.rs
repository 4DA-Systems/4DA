// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Lockfile readers (single-engine C0.3): every installed
//! `(ecosystem, name, version, scope)` a directory's lockfiles name, plus an
//! account of every lockfile that could NOT be read.
//!
//! Measured 2026-10-10 on 19 public repositories against osv-scanner,
//! cargo-audit, pip-audit and govulncheck, the per-format parsers that lived
//! in the app's `scanner.rs` lost installed packages before OSV was ever
//! asked: nested `node_modules` copies were dropped, yarn berry's `version:`
//! line was not recognised, bun.lock / uv.lock / Pipfile.lock and
//! `requirements-*.txt` had no reader at all, and Go read every version
//! go.sum has ever hashed as installed (precision 7.9%). Each of those was
//! silent. This module reads them and reports what it could not
//! ([`LockfileOutcome`]).
//!
//! No file is opened here. A directory's files, and a lockfile's companions
//! (`go.sum`, `-r` includes), come through the host's [`FileSource`]; the
//! app's is `std::fs` (`ace::lockfile::StdFs`), [`MemFs`] holds them in
//! memory. The directory walk (which directories to read) stays in the app.
//!
//! - [`read_dir`]: the outcomes for ONE directory.
//! - [`read_text`]: one lockfile's text, read as a given format.
//! - [`probe`]: the first dependency file a directory holds.

pub mod cargo_facts;
pub mod dep_scope;
pub mod flat;
pub mod graph;
pub mod npm_bundle;
pub mod source;

mod bun;
mod go;
mod npm;
mod python;
mod ruby;
mod yarn;

pub use graph::DependencyEdge;
pub use python::requirements_files;
pub use source::{Entry, FileSource, MemFs};
pub use yarn::normalize_descriptor;

use source::{cmp_paths, extension};

/// A lockfile format this module reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockFormat {
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
    pub fn ecosystem(self) -> &'static str {
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
pub enum DepScope {
    /// The lockfile records nothing about dev vs runtime.
    Unknown,
    /// Reached only from dev roots.
    Dev,
    /// Reached from a runtime root.
    Runtime,
}

impl DepScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Dev => "dev",
            Self::Runtime => "runtime",
        }
    }

    /// Two lockfiles (or two paths in one) that disagree: runtime wins over
    /// dev, and any recorded scope over unknown.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

/// One installed copy.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub scope: DepScope,
    /// The copy the project itself resolves the name to (the hoisted npm
    /// copy, the go.mod requirement). The collapsed per-name row keeps it.
    pub primary: bool,
    /// The copy sits where a manifest declaration of its name resolves: the
    /// hoisted `node_modules/<name>`, a workspace member's own
    /// `<member>/node_modules/<name>`, a pnpm importer's pin, a package the
    /// project's own crates list in Cargo.lock. Only such a copy can be
    /// DIRECT; a nested copy of a declared name is some dependency's
    /// transitive. True by default: formats that hold one version per name
    /// (Python, Go, Ruby, Composer) resolve every copy. yarn.lock records no
    /// position, so its copies start false and carry [`Self::specs`].
    pub top_level: bool,
    /// The workspace member whose OWN slot holds a [`Self::top_level`] copy
    /// (npm: its directory, `packages/ui`; bun: its package name) — only
    /// that member's declarations resolve to it. `None` for the hoisted /
    /// root slot, which any workspace package may resolve to.
    pub owner: Option<String>,
    /// yarn.lock only: the descriptors (`name@range`) the entry resolves,
    /// `npm:` protocol removed, for matching a manifest's declared ranges.
    pub specs: Vec<String>,
}

impl LockedPackage {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            scope: DepScope::Unknown,
            primary: false,
            top_level: true,
            owner: None,
            specs: Vec::new(),
        }
    }

    #[must_use]
    pub fn scoped(mut self, scope: DepScope) -> Self {
        self.scope = scope;
        self
    }

    #[must_use]
    pub fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }

    #[must_use]
    pub fn top_level(mut self, top_level: bool) -> Self {
        self.top_level = top_level;
        self
    }

    #[must_use]
    pub fn owned_by(mut self, owner: Option<String>) -> Self {
        self.owner = owner;
        self
    }
}

/// A lockfile that was read.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LockfileRead {
    /// The file, as the host spells it.
    pub path: String,
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
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockfileOutcome {
    Read(LockfileRead),
    /// Present, but unreadable or unparseable.
    Failed {
        path: String,
        format: Option<LockFormat>,
        reason: String,
    },
    /// A dependency file of an ecosystem or format 4DA does not read.
    Unsupported {
        path: String,
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
pub fn probe(fs: &dyn FileSource, dir: &str) -> Option<String> {
    READERS
        .iter()
        .map(|(name, _)| *name)
        .chain(UNSUPPORTED.iter().map(|(name, _)| *name))
        .chain(std::iter::once("go.sum"))
        .map(|name| fs.join(dir, name))
        .find(|p| fs.is_file(p))
        .or_else(|| requirements_files(fs, dir).into_iter().next())
        .or_else(|| dotnet_project_files(fs, dir).into_iter().next())
}

/// Read every dependency file in `dir`. Never fails: a file that cannot be
/// read becomes a [`LockfileOutcome::Failed`].
pub fn read_dir(fs: &dyn FileSource, dir: &str) -> Vec<LockfileOutcome> {
    let mut out = Vec::new();
    for (name, format) in READERS {
        let path = fs.join(dir, name);
        if fs.is_file(&path) {
            out.push(read_file(fs, path, *format));
        }
    }
    let go_sum = fs.join(dir, "go.sum");
    if fs.is_file(&go_sum) && !fs.is_file(&fs.join(dir, "go.mod")) {
        out.push(LockfileOutcome::Failed {
            path: go_sum,
            format: None,
            reason: "go.sum without go.mod: the build list cannot be resolved".to_string(),
        });
    }
    let requirements = requirements_files(fs, dir);
    if !requirements.is_empty() {
        out.extend(python::read_requirements_set(fs, &requirements));
    }
    for (name, kind) in UNSUPPORTED {
        let path = fs.join(dir, name);
        if fs.is_file(&path) {
            out.push(LockfileOutcome::Unsupported { path, kind });
        }
    }
    // A .NET project is a `*.csproj` / `*.fsproj` / `*.vbproj`; without
    // `packages.lock.json` (already listed above) it was never reported at
    // all, so it read as having no dependencies (audit A2, 2026-10-12).
    if !fs.is_file(&fs.join(dir, "packages.lock.json")) {
        out.extend(dotnet_project_files(fs, dir).into_iter().map(|path| {
            LockfileOutcome::Unsupported {
                path,
                kind: DOTNET_PROJECT_KIND,
            }
        }));
    }
    out
}

const DOTNET_PROJECT_KIND: &str = "NuGet (.NET project, installed versions not resolved)";

/// The .NET project files directly in `dir`, sorted for a stable report.
fn dotnet_project_files(fs: &dyn FileSource, dir: &str) -> Vec<String> {
    let mut files: Vec<String> = fs
        .list(dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| {
            e.is_file
                && extension(&e.name).is_some_and(|x| {
                    matches!(
                        x.to_ascii_lowercase().as_str(),
                        "csproj" | "fsproj" | "vbproj"
                    )
                })
        })
        .map(|e| fs.join(dir, &e.name))
        .collect();
    files.sort_by(|a, b| cmp_paths(a, b));
    files
}

fn read_file(fs: &dyn FileSource, path: String, format: LockFormat) -> LockfileOutcome {
    match fs.read(&path) {
        Ok(content) => read_text(fs, &path, format, &content),
        Err(e) => LockfileOutcome::Failed {
            path,
            format: Some(format),
            reason: format!("unreadable: {e}"),
        },
    }
}

/// Parse lockfile text as `format`. `path` names the file in the outcome; the
/// Go and requirements readers also resolve companions (`go.sum`, `-r`
/// includes) against it, through `fs`.
pub fn read_text(
    fs: &dyn FileSource,
    path: &str,
    format: LockFormat,
    content: &str,
) -> LockfileOutcome {
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
        LockFormat::Requirements => Ok(python::read_requirements(fs, path, content)),
        LockFormat::GoMod => Ok(go::read_go_module(fs, path, content)),
        LockFormat::Gemfile => Ok(ruby::read_gemfile_lock(content)),
        LockFormat::Composer => serde_json::from_str::<serde_json::Value>(content)
            .map(|_| plain(format, flat::composer_lock(content)))
            .map_err(|e| format!("invalid JSON: {e}")),
    };
    match parsed {
        Ok(mut read) => {
            read.path = path.to_string();
            LockfileOutcome::Read(read)
        }
        Err(reason) => LockfileOutcome::Failed {
            path: path.to_string(),
            format: Some(format),
            reason,
        },
    }
}

/// A read whose packages carry no scope and no graph.
pub fn plain(format: LockFormat, packages: Vec<(String, String)>) -> LockfileRead {
    let mut seen = std::collections::HashSet::new();
    LockfileRead {
        path: String::new(),
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
/// dependencies carry no `source`). Used by the conformance API; the app's
/// store path keeps its own Cargo processor (`ace_commands::dependencies`).
fn read_cargo_lock(content: &str) -> LockfileRead {
    let local = cargo_facts::local_packages(content);
    let all = flat::cargo_lock(content);
    let total = all.len();
    let packages: Vec<(String, String)> = all.into_iter().filter(|k| !local.contains(k)).collect();
    let mut read = plain(LockFormat::Cargo, packages);
    read.non_registry_entries = total - read.packages.len();
    read
}

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod tests;

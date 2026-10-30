// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! How the fix reaches a TRANSITIVE copy: the one rule the Upgrade Plan,
//! Preemption and the Brief all apply (trajectory 2026-10-10, gate G2).
//! Moved from the app's `osv/fix_path.rs` (single-engine C0.4), which
//! re-exports it and keeps the halves that look at the disk: which tool owns
//! a project's lockfile (`Manager::detect`, `NpmLockfile::detect`), finding
//! the `package-lock.json` above a project, and cargo's registry checkout.
//!
//! The question is always the same — the copy's target is known
//! (`fix_target`), the copy is not declared by the project, so does
//! refreshing the lockfile reach the target, or must the parent that pulls
//! it in move first? Two copies of the answer had drifted: the Brief said
//! "refresh" whenever the jump was semver-compatible, the Upgrade Plan said
//! "update the parent, or refresh the lockfile if its requirement admits it"
//! for every Cargo and pnpm copy (their lockfiles record resolutions, not
//! requirements). A mechanical oracle on public repos (2026-10-10) applied
//! the recommendation, re-resolved and re-scanned: a refresh cleared mio
//! 0.8.0 (Cargo), braces 3.0.2 (pnpm) and minimist 1.2.5 (npm) — all three
//! had been left "waiting on upstream".
//!
//! The evidence, strongest first:
//! 1. the parent's own requirement on the package — the lockfile edge
//!    (`package-lock.json` records it), else the parent's installed manifest
//!    (`node_modules/.../package.json`, or cargo's registry checkout of the
//!    parent release, which the build already extracted). The manifest is
//!    read through the host ([`crate::needs`]): [`installed_parent_requirements`];
//! 2. otherwise semver compatibility between the installed copy and the
//!    target: a caret requirement — npm's and Cargo's default — that admits
//!    the installed copy admits a compatible target. This is an inference,
//!    and every rendering says so.
//!
//! Commands are named only for tools whose refresh this module can stand
//! behind (Principle 5): `npm update`, `pnpm update --depth Infinity`,
//! `yarn up -R` (berry) and `cargo update --precise`. Yarn v1 has no command
//! that moves one transitive copy in place, so none is named.

use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::fix_target::{upgrade_type, UpgradeType};
use crate::lockfile::npm_bundle::{is_dependency_key, npm_lock_entry_bundled};
use crate::needs::{lookup, Host, Lookup, Provided};
use crate::npm_range::requirement_admits;
use crate::range::parse_version;

/// How a refresh verdict was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// The parent's own requirement on the package was read.
    Requirement,
    /// No requirement could be read; inferred from semver compatibility.
    Semver,
}

/// Does refreshing the lockfile reach the target?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refresh {
    /// Yes: no manifest changes, the lockfile moves inside existing ranges.
    Enough(Basis),
    /// No: a parent's requirement (or a semver-incompatible jump) keeps the
    /// copy on its line until that parent moves.
    ParentMustMove(Basis),
    /// Neither the requirement nor the versions could be read.
    Unknown,
}

impl Refresh {
    pub fn is_enough(self) -> bool {
        matches!(self, Self::Enough(_))
    }
}

/// The requirement syntax a recorded requirement is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// npm/pnpm/yarn: a bare version is an exact pin, `||` unions, spaces AND.
    Npm,
    /// Cargo: a bare version is a caret requirement, commas AND.
    Cargo,
}

/// A parent's requirement on the package, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub text: String,
    pub dialect: Dialect,
}

impl Requirement {
    pub fn npm(text: &str) -> Self {
        Self {
            text: text.trim().to_string(),
            dialect: Dialect::Npm,
        }
    }

    pub fn cargo(text: &str) -> Self {
        Self {
            text: text.trim().to_string(),
            dialect: Dialect::Cargo,
        }
    }

    /// `None` when the requirement or the version cannot be read.
    pub fn admits(&self, version: &str) -> Option<bool> {
        match self.dialect {
            Dialect::Npm => requirement_admits(&self.text, version),
            Dialect::Cargo => {
                let v = parse_version(version)?;
                VersionReq::parse(&self.text).ok().map(|r| r.matches(&v))
            }
        }
    }
}

/// THE fix-path rule for a transitive copy at `installed` whose target is
/// `target`, given every requirement read for the parent(s) that pull it in.
/// Any requirement that excludes the target sends the fix to the parent;
/// all of them admitting it makes a refresh enough; nothing readable falls
/// back to semver compatibility.
pub fn refresh_verdict(installed: &str, target: &str, requirements: &[Requirement]) -> Refresh {
    let read: Vec<bool> = requirements
        .iter()
        .filter_map(|r| r.admits(target))
        .collect();
    if read.iter().any(|admits| !admits) {
        return Refresh::ParentMustMove(Basis::Requirement);
    }
    if !read.is_empty() && read.len() == requirements.len() {
        return Refresh::Enough(Basis::Requirement);
    }
    match upgrade_type(installed, target) {
        Some(UpgradeType::Major) => Refresh::ParentMustMove(Basis::Semver),
        Some(_) => Refresh::Enough(Basis::Semver),
        None => Refresh::Unknown,
    }
}

/// The package manager that owns a project's lockfile for one ecosystem.
/// Which one a project has is the host's to detect (the app's
/// `Manager::detect` probes the directories above it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Manager {
    Npm,
    Pnpm,
    /// A pnpm workspace root (`pnpm-workspace.yaml`): `pnpm update` without
    /// `-r` touches only the root project's own tree.
    PnpmWorkspace,
    /// Yarn 1 (`yarn.lock` without a `__metadata:` header).
    YarnClassic,
    /// Yarn 2+ (berry).
    YarnBerry,
    Cargo,
}

impl Manager {
    /// The lockfile records each parent's REQUIREMENT on its children (only
    /// `package-lock.json` does; the others record resolved versions).
    pub fn records_requirements(self) -> bool {
        self == Self::Npm
    }

    /// The one command that refreshes `package` inside the existing ranges,
    /// or `None` when this tool has no such command (yarn 1).
    pub fn refresh_command(self, package: &str, installed: &str, target: &str) -> Option<String> {
        match self {
            Self::Npm => Some(format!("npm update {package}")),
            Self::Pnpm => Some(format!("pnpm update {package} --depth Infinity")),
            Self::PnpmWorkspace => Some(format!("pnpm update -r {package} --depth Infinity")),
            Self::YarnBerry => Some(format!("yarn up -R {package}")),
            Self::YarnClassic => None,
            Self::Cargo => Some(format!(
                "cargo update -p {package}@{installed} --precise {target}"
            )),
        }
    }

    /// Where this tool lets a project force a transitive copy past its
    /// parent's requirement, worded to follow "with". `None` for Cargo: no
    /// manifest key moves a crate outside the line its parent requires.
    pub fn override_mechanism(self) -> Option<&'static str> {
        match self {
            Self::Npm => Some("an `overrides` entry in package.json (npm)"),
            Self::Pnpm | Self::PnpmWorkspace => Some("a `pnpm.overrides` entry in package.json"),
            Self::YarnClassic | Self::YarnBerry => {
                Some("a `resolutions` entry in package.json (yarn)")
            }
            Self::Cargo => None,
        }
    }
}

/// The override mechanism for a project's tool, or, when the tool is not
/// known, every npm-family one (`ecosystem_norm` is OSV-canonical). `None`
/// for an ecosystem with no such mechanism, and for a `bundled` copy: npm
/// `overrides` (and pnpm's and yarn's) do not reach a dependency that ships
/// inside its owner's tarball (`crate::lockfile::npm_bundle::BUNDLED_SUFFIX`); only the owner can move.
pub fn override_hint(
    manager: Option<Manager>,
    ecosystem_norm: &str,
    bundled: bool,
) -> Option<String> {
    if bundled {
        return None;
    }
    match manager {
        Some(m) => m.override_mechanism().map(str::to_string),
        None if ecosystem_norm == "npm" => Some(
            "an override in package.json (npm `overrides`, pnpm `pnpm.overrides` or yarn `resolutions`)"
                .to_string(),
        ),
        None => None,
    }
}

/// The npm-family lockfile in one directory (no ascent). The walk runs the
/// package-lock, then the pnpm, then the yarn processor, and each REPLACES
/// the project's npm instance set — so the last one present wrote the pins.
/// Shared with the app's `evidence::install_drift`, which names its install
/// command. Which one a directory holds is the host's to detect (the app's
/// `NpmLockfile::detect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpmLockfile {
    Npm,
    Pnpm,
    Yarn,
}

impl NpmLockfile {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Npm => "package-lock.json",
            Self::Pnpm => "pnpm-lock.yaml",
            Self::Yarn => "yarn.lock",
        }
    }

    /// The command that makes `node_modules` match this lockfile again.
    pub fn install_command(self) -> &'static str {
        match self {
            Self::Npm => "npm ci",
            Self::Pnpm => "pnpm install",
            Self::Yarn => "yarn install",
        }
    }
}

/// The versions of `child` that the installed `parent` `parent_version`
/// resolves to in a `package-lock.json` v2/v3 tree, by npm's own lookup: the
/// parent's folder's `node_modules`, then each enclosing folder's, up to the
/// root. A range alone cannot tell which copy a parent holds: NodeGoat's
/// needle 2.2.4 (`^2.1.2`) admits debug 2.2.0 but resolves the hoisted 2.6.9,
/// and was told `npm update debug` for a copy it never held (audit E8).
///
/// Copies of `parent` bundled inside a dependency are skipped: what they
/// resolve is frozen in that bundle, and the bundled edge already names its
/// owner. `None` when the lockfile has no `packages` map (v1) or holds no
/// matching copy of `parent` that declares `child` (a database older than the
/// lockfile on disk), so the caller keeps its range-based attribution.
pub fn npm_lock_resolutions(
    lock: &serde_json::Value,
    parent: &str,
    parent_version: &str,
    child: &str,
) -> Option<Vec<String>> {
    let pkgs = lock.get("packages")?.as_object()?;
    let mut matched = false;
    let mut out: Vec<String> = Vec::new();
    for (key, entry) in pkgs {
        let name = key
            .rsplit_once("node_modules/")
            .map_or(key.as_str(), |(_, n)| n);
        if !is_dependency_key(key)
            || !name.eq_ignore_ascii_case(parent)
            || entry.get("version").and_then(serde_json::Value::as_str) != Some(parent_version)
            || npm_lock_entry_bundled(pkgs, key)
        {
            continue;
        }
        let declares = ["dependencies", "optionalDependencies", "peerDependencies"]
            .iter()
            .any(|s| entry.get(*s).and_then(|d| d.get(child)).is_some());
        if !declares {
            continue;
        }
        matched = true;
        let mut base = key.as_str();
        let resolved = loop {
            let candidate = if base.is_empty() {
                format!("node_modules/{child}")
            } else {
                format!("{base}/node_modules/{child}")
            };
            if let Some(found) = pkgs.get(&candidate) {
                break found.get("version").and_then(serde_json::Value::as_str);
            }
            if base.is_empty() {
                break None;
            }
            base = base.rsplit_once("/node_modules/").map_or("", |(b, _)| b);
        };
        if let Some(v) = resolved {
            if !out.iter().any(|o| o == v) {
                out.push(v.to_string());
            }
        }
    }
    matched.then_some(out)
}

/// The requirements `parent` `parent_version` places on `child`, read from
/// the parent's installed manifest: cargo's registry checkout of that release
/// ([`crate::needs::Need::CrateManifest`]), or the copy under the project's
/// `node_modules` at `project_dir` (the host's spelling). Empty when not on
/// disk — the caller then falls back to semver compatibility, and says so.
/// What the host has not answered yet is recorded on `host` and read as absent.
pub fn installed_parent_requirements(
    manager: Manager,
    project_dir: &str,
    parent: &str,
    parent_version: &str,
    child: &str,
    host: &Host<'_>,
) -> Vec<Requirement> {
    match manager {
        Manager::Cargo => host
            .crate_manifest(parent, parent_version)
            .map(|manifest| {
                cargo_requirements(manifest, child)
                    .iter()
                    .map(|r| Requirement::cargo(r))
                    .collect()
            })
            .unwrap_or_default(),
        _ => node_modules_manifest(project_dir, parent, parent_version, host)
            .and_then(|pkg| npm_requirement(&pkg, child))
            .map(|r| vec![Requirement::npm(&r)])
            .unwrap_or_default(),
    }
}

/// [`installed_parent_requirements`] as one round-trip step: the
/// requirements, or the files the host must read first.
pub fn installed_parent_requirements_lookup(
    manager: Manager,
    project_dir: &str,
    parent: &str,
    parent_version: &str,
    child: &str,
    provided: &Provided,
    join: &dyn Fn(&str, &str) -> String,
) -> Lookup<Vec<Requirement>> {
    lookup(provided, join, |host| {
        installed_parent_requirements(manager, project_dir, parent, parent_version, child, host)
    })
}

/// The parent's `package.json` at exactly `version` under `project_dir`'s
/// `node_modules`: the hoisted copy, or pnpm's virtual store entry.
fn node_modules_manifest(
    project_dir: &str,
    name: &str,
    version: &str,
    host: &Host<'_>,
) -> Option<serde_json::Value> {
    let nm = host.join(project_dir, "node_modules");
    let mut candidates: Vec<String> = vec![host.join(&host.join(&nm, name), "package.json")];
    let store_key = format!("{}@{version}", name.replace('/', "+"));
    let store = host.join(&nm, ".pnpm");
    if let Some(entries) = host.listing(&store) {
        for file in entries {
            // `name@1.2.3` or `name@1.2.3_peer…` / `name@1.2.3(peer…)`.
            let rest = file.strip_prefix(store_key.as_str());
            if rest.is_some_and(|r| r.is_empty() || r.starts_with(['_', '('])) {
                let entry = host.join(&store, file);
                let pkg = host.join(&host.join(&entry, "node_modules"), name);
                candidates.push(host.join(&pkg, "package.json"));
            }
        }
    }
    candidates.into_iter().find_map(|path| {
        let pkg: serde_json::Value = serde_json::from_str(host.manifest(&path)?).ok()?;
        let installed = pkg.get("version")?.as_str()?;
        (installed.trim_start_matches('v') == version.trim_start_matches('v')).then_some(pkg)
    })
}

/// The requirement a `package.json` places on `child` in the sections that
/// resolve with the package (`dependencies`, `optionalDependencies`): what
/// [`installed_parent_requirements`] reads from an installed parent's
/// manifest.
pub fn npm_requirement(pkg: &serde_json::Value, child: &str) -> Option<String> {
    ["dependencies", "optionalDependencies"]
        .iter()
        .find_map(|section| {
            pkg.get(section)?
                .as_object()?
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(child))
                .and_then(|(_, req)| req.as_str())
                .map(str::to_string)
        })
}

/// Every requirement a crate's `Cargo.toml` places on `child` in the tables
/// that resolve for a consumer (`[dependencies]`, `[build-dependencies]` and
/// their `[target.*]` forms; never `[dev-dependencies]`). Reads both the
/// inline form (`mio = "0.8"`, `mio = { version = "0.8" }`) and the table form
/// crates.io normalizes to (`[dependencies.mio]` + `version = "0.8"`), and a
/// renamed dependency (`package = "mio"`). Line-based on purpose: the crate
/// has no TOML parser, and a manifest this cannot read yields nothing.
pub fn cargo_requirements(manifest: &str, child: &str) -> Vec<String> {
    let want = crate_key(child);
    let mut out = Vec::new();
    let mut table: Option<CargoTable> = None;
    let flush = |t: Option<CargoTable>, out: &mut Vec<String>| {
        if let Some(CargoTable::Dep {
            name,
            package,
            version,
        }) = t
        {
            if crate_key(package.as_deref().unwrap_or(&name)) == want {
                out.extend(version);
            }
        }
    };
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(table.take(), &mut out);
            table = cargo_table(line);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (unquote(key.trim()), value.trim());
        match table.as_mut() {
            Some(CargoTable::Deps) => {
                if let Some(req) = inline_dep(key, value, &want) {
                    out.push(req);
                }
            }
            Some(CargoTable::Dep {
                version, package, ..
            }) => match key {
                "version" => *version = Some(unquote(value).to_string()),
                "package" => *package = Some(unquote(value).to_string()),
                _ => {}
            },
            None => {}
        }
    }
    flush(table, &mut out);
    out
}

enum CargoTable {
    /// `[dependencies]`-style: one `name = spec` per line.
    Deps,
    /// `[dependencies.name]`-style: the fields of one dependency.
    Dep {
        name: String,
        package: Option<String>,
        version: Option<String>,
    },
}

fn cargo_table(header: &str) -> Option<CargoTable> {
    let inner = header.trim_start_matches('[').trim_end_matches(']').trim();
    // `target.'cfg(unix)'.dependencies.mio` -> drop the target prefix.
    let inner = match inner.strip_prefix("target.") {
        Some(rest) => {
            let rest = rest.trim_start_matches(['"', '\'']);
            let close = rest.find(['"', '\''])?;
            rest[close + 1..].trim_start_matches('.')
        }
        None => inner,
    };
    let (section, name) = match inner.split_once('.') {
        Some((section, name)) => (section, Some(unquote(name))),
        None => (inner, None),
    };
    if section != "dependencies" && section != "build-dependencies" {
        return None;
    }
    Some(match name {
        Some(name) => CargoTable::Dep {
            name: name.to_string(),
            package: None,
            version: None,
        },
        None => CargoTable::Deps,
    })
}

/// `mio = "0.8"` or `mio = { version = "0.8", package = "x" }` when it names
/// the wanted crate.
fn inline_dep(key: &str, value: &str, want: &str) -> Option<String> {
    if let Some(body) = value.strip_prefix('{') {
        let body = body.trim_end_matches('}');
        let field = |name: &str| {
            body.split(',').find_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                (k.trim() == name).then(|| unquote(v.trim()).to_string())
            })
        };
        let package = field("package");
        (crate_key(package.as_deref().unwrap_or(key)) == want)
            .then(|| field("version"))
            .flatten()
    } else {
        (crate_key(key) == want).then(|| unquote(value).to_string())
    }
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches('"').trim_matches('\'')
}

/// Cargo treats `-` and `_` in crate names as the same crate.
fn crate_key(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

#[cfg(test)]
#[path = "fix_path_tests.rs"]
mod tests;

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! How the fix reaches a TRANSITIVE copy: the one rule the Upgrade Plan,
//! Preemption and the Brief all apply (trajectory 2026-10-10, gate G2).
//!
//! The question is always the same — the copy's target is known
//! (`osv::fix_target`), the copy is not declared by the project, so does
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
//!    parent release, which the build already extracted);
//! 2. otherwise semver compatibility between the installed copy and the
//!    target: a caret requirement — npm's and Cargo's default — that admits
//!    the installed copy admits a compatible target. This is an inference,
//!    and every rendering says so.
//!
//! Commands are named only for tools whose refresh this module can stand
//! behind (Principle 5): `npm update`, `pnpm update --depth Infinity`,
//! `yarn up -R` (berry) and `cargo update --precise`. Yarn v1 has no command
//! that moves one transitive copy in place, so none is named.

use std::path::{Path, PathBuf};

use semver::VersionReq;

use super::fix_target::{upgrade_type, UpgradeType};
use super::matching::parse_version;

/// How a refresh verdict was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// The parent's own requirement on the package was read.
    Requirement,
    /// No requirement could be read; inferred from semver compatibility.
    Semver,
}

/// Does refreshing the lockfile reach the target?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// How far up from a project directory a workspace lockfile is looked for.
const MAX_LOCKFILE_ASCENT: usize = 6;

/// The directory on disk that a project path names. Findings carry project
/// paths lowercased (`fourda_depengine::matching::normalize_project_path`),
/// which still opens on Windows and macOS but not on a case-sensitive
/// filesystem: `/home/runner/work/4DA/4DA/...` lowercased does not exist, so
/// every manifest read from it failed and no command was named (#972's queue
/// run, 2026-10-17). Each missing component is matched case-insensitively
/// against its parent's entries; two entries that differ only by case are
/// ambiguous and resolve to nothing.
pub(crate) fn on_disk(path: &Path) -> Option<std::path::PathBuf> {
    use std::path::{Component, PathBuf};
    if path.is_dir() {
        return Some(path.to_path_buf());
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        let Component::Normal(name) = component else {
            out.push(component.as_os_str());
            continue;
        };
        let exact = out.join(name);
        if exact.exists() {
            out = exact;
            continue;
        }
        let want = name.to_string_lossy().to_lowercase();
        let base = if out.as_os_str().is_empty() {
            Path::new(".")
        } else {
            out.as_path()
        };
        let mut hits = std::fs::read_dir(base)
            .ok()?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().to_lowercase() == want);
        let found = hits.next()?.path();
        if hits.next().is_some() {
            return None;
        }
        out = found;
    }
    out.is_dir().then_some(out)
}

impl Manager {
    /// The manager whose lockfile holds `ecosystem_norm` (OSV-canonical)
    /// packages for the project at `dir`, nearest directory first. `None`
    /// for an ecosystem this module names no command for, or no lockfile.
    pub fn detect(dir: &Path, ecosystem_norm: &str) -> Option<Self> {
        let resolved = on_disk(dir);
        let mut here = Some(resolved.as_deref().unwrap_or(dir));
        for _ in 0..=MAX_LOCKFILE_ASCENT {
            let d = here?;
            let found = match ecosystem_norm {
                "crates.io" => d.join("Cargo.lock").is_file().then_some(Self::Cargo),
                "npm" => NpmLockfile::detect(d).map(|kind| match kind {
                    NpmLockfile::Npm => Self::Npm,
                    NpmLockfile::Pnpm if d.join("pnpm-workspace.yaml").is_file() => {
                        Self::PnpmWorkspace
                    }
                    NpmLockfile::Pnpm => Self::Pnpm,
                    NpmLockfile::Yarn if is_yarn_berry(&d.join("yarn.lock")) => Self::YarnBerry,
                    NpmLockfile::Yarn => Self::YarnClassic,
                }),
                _ => return None,
            };
            if found.is_some() {
                return found;
            }
            here = d.parent();
        }
        None
    }

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
/// inside its owner's tarball ([`BUNDLED_SUFFIX`]); only the owner can move.
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
/// Shared with `evidence::install_drift`, which names its install command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpmLockfile {
    Npm,
    Pnpm,
    Yarn,
}

impl NpmLockfile {
    pub fn detect(dir: &Path) -> Option<Self> {
        [Self::Yarn, Self::Pnpm, Self::Npm]
            .into_iter()
            .find(|kind| dir.join(kind.file_name()).is_file())
    }

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

/// The parsed `package-lock.json` that owns the npm tree of the project at
/// `dir`: the nearest one up the same ascent [`Manager::detect`] takes (a
/// workspace member's lockfile sits at the workspace root). Parsed once per
/// file and kept while its modification time and size are unchanged: the Upgrade
/// Plan asks once per vulnerable package, and a lockfile can be megabytes.
pub(crate) fn npm_lock_tree(dir: &Path) -> Option<std::sync::Arc<serde_json::Value>> {
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock};
    type Entry = (std::time::SystemTime, u64, Arc<serde_json::Value>);
    static CACHE: LazyLock<parking_lot::Mutex<HashMap<PathBuf, Entry>>> =
        LazyLock::new(Default::default);
    const MAX_CACHED: usize = 16;

    let mut here = Some(dir);
    let mut file = None;
    for _ in 0..=MAX_LOCKFILE_ASCENT {
        let d = here?;
        match NpmLockfile::detect(d) {
            Some(NpmLockfile::Npm) => {
                file = Some(d.join(NpmLockfile::Npm.file_name()));
                break;
            }
            Some(_) => return None, // pnpm or yarn owns this tree
            None => here = d.parent(),
        }
    }
    let file = file?;
    let meta = std::fs::metadata(&file).ok()?;
    let stamp = (meta.modified().ok()?, meta.len());
    if let Some((modified, len, tree)) = CACHE.lock().get(&file) {
        if (*modified, *len) == stamp {
            return Some(Arc::clone(tree));
        }
    }
    let tree: Arc<serde_json::Value> =
        Arc::new(serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()?);
    let mut cache = CACHE.lock();
    if cache.len() >= MAX_CACHED {
        cache.clear();
    }
    cache.insert(file, (stamp.0, stamp.1, Arc::clone(&tree)));
    Some(tree)
}

/// A `package-lock.json` v2/v3 key that installs a DEPENDENCY (under some
/// `node_modules/`), as opposed to the root (`""`) or a workspace member's
/// folder (`packages/web`).
fn is_dependency_key(key: &str) -> bool {
    key.starts_with("node_modules/") || key.contains("/node_modules/")
}

/// The key of the package a bundled entry at `key` ships inside: the nearest
/// enclosing entry that is not itself `inBundle` (a level the lockfile does
/// not list is passed over). `None` when no enclosing entry is listed.
pub(crate) fn npm_bundle_owner<'a>(
    pkgs: &'a serde_json::Map<String, serde_json::Value>,
    key: &'a str,
) -> Option<&'a str> {
    let mut at = key;
    loop {
        let (enclosing, _) = at.rsplit_once("/node_modules/")?;
        at = enclosing;
        match pkgs.get(enclosing) {
            Some(entry)
                if !entry
                    .get("inBundle")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false) =>
            {
                return Some(enclosing)
            }
            _ => {}
        }
    }
}

/// Whether the entry at `key` is a copy bundled inside a DEPENDENCY's tarball:
/// `inBundle`, with a dependency as its bundler. A project's or workspace
/// member's own `bundleDependencies` also set `inBundle` (npm Arborist's
/// `getBundler()` can be the root), but those copies sit in the user's own
/// tree, where `npm update` and overrides reach them like any other.
pub(crate) fn npm_lock_entry_bundled(
    pkgs: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> bool {
    let in_bundle = pkgs
        .get(key)
        .and_then(|e| e.get("inBundle"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    in_bundle && npm_bundle_owner(pkgs, key).is_some_and(is_dependency_key)
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
pub(crate) fn npm_lock_resolutions(
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

fn is_yarn_berry(lockfile: &Path) -> bool {
    std::fs::read_to_string(lockfile)
        .map(|s| s.lines().take(12).any(|l| l.starts_with("__metadata:")))
        .unwrap_or(false)
}

/// The requirements `parent` `parent_version` places on `child`, read from
/// the parent's installed manifest: cargo's registry checkout of that release,
/// or the copy under the project's `node_modules`. Empty when not on disk —
/// the caller then falls back to semver compatibility, and says so.
pub fn installed_parent_requirements(
    manager: Manager,
    project_dir: &Path,
    parent: &str,
    parent_version: &str,
    child: &str,
) -> Vec<Requirement> {
    match manager {
        Manager::Cargo => crate_checkout(parent, parent_version)
            .and_then(|dir| std::fs::read_to_string(dir.join("Cargo.toml")).ok())
            .map(|manifest| {
                cargo_requirements(&manifest, child)
                    .iter()
                    .map(|r| Requirement::cargo(r))
                    .collect()
            })
            .unwrap_or_default(),
        _ => node_modules_manifest(project_dir, parent, parent_version)
            .and_then(|pkg| npm_requirement(&pkg, child))
            .map(|r| vec![Requirement::npm(&r)])
            .unwrap_or_default(),
    }
}

/// cargo's extracted copy of a crates.io release,
/// `$CARGO_HOME/registry/src/<index>/<name>-<version>` — the same checkout
/// `osv::reachability` parses for AD-051 (kept local here while that file is
/// in a peer's change; fold into one helper when it lands).
fn crate_checkout(package: &str, version: &str) -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))?;
    let names = [
        format!("{package}-{version}"),
        format!("{}-{version}", package.to_lowercase()),
    ];
    std::fs::read_dir(home.join("registry").join("src"))
        .ok()?
        .flatten()
        .flat_map(|index| names.iter().map(move |n| index.path().join(n)))
        .find(|dir| dir.join("Cargo.toml").is_file())
}

/// The parent's `package.json` at exactly `version` under `project_dir`'s
/// `node_modules`: the hoisted copy, or pnpm's virtual store entry.
fn node_modules_manifest(
    project_dir: &Path,
    name: &str,
    version: &str,
) -> Option<serde_json::Value> {
    let nm = project_dir.join("node_modules");
    let mut candidates: Vec<PathBuf> = vec![nm.join(name).join("package.json")];
    let store_key = format!("{}@{version}", name.replace('/', "+"));
    if let Ok(entries) = std::fs::read_dir(nm.join(".pnpm")) {
        for entry in entries.flatten() {
            let file = entry.file_name();
            let file = file.to_string_lossy();
            // `name@1.2.3` or `name@1.2.3_peer…` / `name@1.2.3(peer…)`.
            let rest = file.strip_prefix(store_key.as_str());
            if rest.is_some_and(|r| r.is_empty() || r.starts_with(['_', '('])) {
                candidates.push(
                    entry
                        .path()
                        .join("node_modules")
                        .join(name)
                        .join("package.json"),
                );
            }
        }
    }
    candidates.into_iter().find_map(|path| {
        let pkg: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        let installed = pkg.get("version")?.as_str()?;
        (installed.trim_start_matches('v') == version.trim_start_matches('v')).then_some(pkg)
    })
}

/// The requirement a `package.json` places on `child` in the sections that
/// resolve with the package (`dependencies`, `optionalDependencies`). Also
/// read for a project's OWN manifest by Signal's stack-change stream, to say
/// whether a new release is inside the declared range.
pub(crate) fn npm_requirement(pkg: &serde_json::Value, child: &str) -> Option<String> {
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
pub(crate) fn cargo_requirements(manifest: &str, child: &str) -> Vec<String> {
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

/// Marks a `package-lock.json` edge to a BUNDLED copy (`inBundle`): the
/// exact version the bundle owner ships, e.g. `2.2.0 (bundled)`. Every reader
/// here sees an exact pin (the text after `(` is dropped), so no refresh is
/// enough; and since npm `overrides` do not reach bundled dependencies, a
/// bundled link names no override either — only a new owner release moves it.
pub const BUNDLED_SUFFIX: &str = " (bundled)";

/// The edge spec names a bundled copy ([`BUNDLED_SUFFIX`]).
pub fn is_bundled(spec: &str) -> bool {
    spec.ends_with(BUNDLED_SUFFIX)
}

// The npm requirement reader moved into the engine crate (D-4: the held-
// dependency rule reads a release's own ranges with it); re-exported under
// its old path for every caller here.
pub(crate) use fourda_depengine::npm_range::{requirement_admits, strip_spec};

#[cfg(test)]
#[path = "fix_path_tests.rs"]
mod tests;

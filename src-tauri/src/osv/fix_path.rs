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
//!
//! The rule moved into the engine crate (`fourda_depengine::fix_path`,
//! single-engine C0.4) and is re-exported here under its old paths. This
//! module is its host: it finds which tool owns a project's lockfile
//! ([`DetectManager`], [`DetectLockfile`]: the directory ascent and its
//! probes), the `package-lock.json` above a project ([`npm_lock_tree`]) and
//! cargo's registry checkout, and answers the engine's `needs`
//! ([`answer_need`]).

use std::path::{Path, PathBuf};

use fourda_depengine::needs::{resolve, Answer, Need};

pub(crate) use fourda_depengine::fix_path::cargo_requirements;
pub use fourda_depengine::fix_path::{
    override_hint, refresh_verdict, Basis, Dialect, Manager, NpmLockfile, Refresh, Requirement,
};

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

/// [`Manager::detect`](DetectManager::detect): which tool owns a project's
/// lockfile. Host-side (it probes the directories above the project), so it
/// stayed here when `Manager` moved into the engine crate (single-engine
/// C0.4); callers bring this trait into scope and call `Manager::detect` as
/// before.
pub trait DetectManager: Sized {
    /// The manager whose lockfile holds `ecosystem_norm` (OSV-canonical)
    /// packages for the project at `dir`, nearest directory first. `None`
    /// for an ecosystem this module names no command for, or no lockfile.
    fn detect(dir: &Path, ecosystem_norm: &str) -> Option<Self>;
}

impl DetectManager for Manager {
    fn detect(dir: &Path, ecosystem_norm: &str) -> Option<Self> {
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
}

/// [`NpmLockfile::detect`](DetectLockfile::detect): the npm-family lockfile
/// in one directory (no ascent). Host-side, like [`DetectManager`].
pub trait DetectLockfile: Sized {
    fn detect(dir: &Path) -> Option<Self>;
}

impl DetectLockfile for NpmLockfile {
    fn detect(dir: &Path) -> Option<Self> {
        [Self::Yarn, Self::Pnpm, Self::Npm]
            .into_iter()
            .find(|kind| dir.join(kind.file_name()).is_file())
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

fn is_yarn_berry(lockfile: &Path) -> bool {
    std::fs::read_to_string(lockfile)
        .map(|s| s.lines().take(12).any(|l| l.starts_with("__metadata:")))
        .unwrap_or(false)
}

/// The requirements `parent` `parent_version` places on `child`, read from
/// the parent's installed manifest: cargo's registry checkout of that release,
/// or the copy under the project's `node_modules`. Empty when not on disk —
/// the caller then falls back to semver compatibility, and says so. The rule
/// is the engine's (`fourda_depengine::fix_path::installed_parent_requirements`);
/// this runs its `needs` round-trip against the disk.
pub fn installed_parent_requirements(
    manager: Manager,
    project_dir: &Path,
    parent: &str,
    parent_version: &str,
    child: &str,
) -> Vec<Requirement> {
    let Some(dir) = project_dir.to_str() else {
        // Project paths come from the database as text, so every caller's is
        // UTF-8; a directory the engine cannot be handed has nothing to read.
        return Vec::new();
    };
    resolve(
        |provided| {
            fourda_depengine::fix_path::installed_parent_requirements_lookup(
                manager,
                dir,
                parent,
                parent_version,
                child,
                provided,
                &host_join,
            )
        },
        answer_need,
    )
}

/// `dir` joined with `name` the way `std::path` joins them on this platform,
/// so the paths the engine names in its needs are the ones the old reads
/// opened. `dir` and `name` are UTF-8, so the join is too: the lossy
/// conversion never replaces anything.
pub(crate) fn host_join(dir: &str, name: &str) -> String {
    Path::new(dir).join(name).to_string_lossy().into_owned()
}

/// Read what the engine asked for. A file that cannot be read, a directory
/// that cannot be listed and a checkout cargo never extracted all answer
/// "not there", as the reads they replace did.
pub(crate) fn answer_need(need: &Need) -> Answer {
    match need {
        Need::Manifest { path } => Answer::Text(std::fs::read_to_string(path).ok()),
        // Entries whose names are not UTF-8 are skipped: the engine cannot be
        // handed them (`ace::lockfile::StdFs::list` does the same). pnpm names
        // its store entries from package names and versions, which are ASCII.
        Need::Listing { dir } => Answer::Names(std::fs::read_dir(dir).ok().map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })),
        Need::CrateManifest { name, version } => Answer::Text(
            crate_checkout(name, version)
                .and_then(|dir| std::fs::read_to_string(dir.join("Cargo.toml")).ok()),
        ),
        Need::NpmLock { dir } => Answer::Tree(npm_lock_tree(Path::new(dir))),
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

// The `inBundle` rules moved into the engine crate with the package-lock
// edge reader (single-engine C0.3); re-exported under their old paths.
#[cfg(test)]
pub use fourda_depengine::lockfile::npm_bundle::{is_bundled, BUNDLED_SUFFIX};

// The npm requirement reader moved into the engine crate (D-4: the held-
// dependency rule reads a release's own ranges with it); re-exported under
// its old path for every caller here.
pub(crate) use fourda_depengine::npm_range::requirement_admits;

#[cfg(test)]
#[path = "fix_path_tests.rs"]
mod tests;

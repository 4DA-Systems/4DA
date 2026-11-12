// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What 4DA may ask the public package registries about (audit P1).
//!
//! The release checks (the npm, crates.io, PyPI and Go module proxy sources,
//! the registry release facts behind fix targets, the release-changelog
//! lane) send dependency names to public registries. Two rules decide what
//! leaves, and every one of those call sites asks [`may_look_up`]:
//!
//! 1. **Settings → Privacy → Registry lookups** (`privacy.registry_lookups`,
//!    on by default). Off, no registry is contacted at all. Advisory matching
//!    is unaffected: it runs on this machine against the downloaded OSV
//!    archive and sends nothing (`osv::sync`).
//! 2. **Private names are never sent**, whatever the setting:
//!    - Go modules matching `GOPRIVATE`, `GONOPROXY` or `GONOSUMDB` (the
//!      environment, then the `go env -w` file), with Go's own pattern rules;
//!      a module a `go.mod` replaces with a local directory; and any module
//!      a scanned `go.mod` declares (the user's own code).
//!    - npm packages in a scope an `.npmrc` (the project's and its parents',
//!      and `~/.npmrc`) routes to a registry other than the public one; every
//!      npm package of a project whose `.npmrc` sets a non-public default
//!      `registry=`, and every npm package when `~/.npmrc` or
//!      `npm_config_registry` does.
//!    - Cargo crates whose `Cargo.lock` source is not crates.io (git, another
//!      registry, a path or workspace member), crates a `Cargo.toml` takes from
//!      `path`, `git` or another `registry`, and the project's own crate.
//!    - PyPI packages a `uv.lock` / `poetry.lock` takes from anywhere but
//!      pypi.org, packages a requirements file takes from a URL or path, the
//!      project's own package, every PyPI package of a project whose
//!      requirements or `pyproject.toml` name another index, and every PyPI
//!      package when the user's pip / uv configuration or environment does.
//!
//! A dependency that is not looked up is not checked: the readiness record
//! carries both facts ([`LookupStatus`]) so no surface reads it as current.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ecosystem::Ecosystem;

/// How long a computed set of private names is reused before the files are
/// read again.
const REFRESH: Duration = Duration::from_mins(10);

/// How far above a project to look for `.npmrc` files (as `ace::npm_local`).
const MAX_NPMRC_ANCESTORS: usize = 12;

/// The registries that ARE the public default (npm).
const NPM_PUBLIC_HOSTS: &[&str] = &["registry.npmjs.org", "registry.yarnpkg.com"];

/// The hosts of the public Python Package Index.
const PYPI_PUBLIC_HOSTS: &[&str] = &["pypi.org", "pypi.python.org", "files.pythonhosted.org"];

static LOOKUPS_ENABLED: AtomicBool = AtomicBool::new(true);

/// Mirror `privacy.registry_lookups` for lock-free reads. Called whenever
/// settings are loaded or saved (`settings::manager`).
pub(crate) fn publish_registry_lookups(enabled: bool) {
    LOOKUPS_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Whether Settings → Privacy → Registry lookups is on.
pub(crate) fn registry_lookups_enabled() -> bool {
    LOOKUPS_ENABLED.load(Ordering::SeqCst)
}

/// Whether 4DA may send `name` to `ecosystem`'s public registry: lookups are
/// on and the name is not private. Ecosystems without a registry lookup
/// answer the setting alone.
pub(crate) fn may_look_up(ecosystem: &str, name: &str) -> bool {
    // The switch first: off, not even the private set is computed.
    registry_lookups_enabled() && allowed(true, &private_names(), ecosystem, name)
}

/// The names of `names` that may be sent to `ecosystem`'s public registry:
/// none while lookups are off, never a private one. The registry sources
/// call this right before each fetch, so turning the switch off (or adding a
/// `GOPRIVATE` pattern) takes effect on the next request.
pub(crate) fn lookable(ecosystem: &str, names: &[String]) -> Vec<String> {
    if !registry_lookups_enabled() {
        return Vec::new();
    }
    let private = private_names();
    let kept: Vec<String> = names
        .iter()
        .filter(|n| allowed(true, &private, ecosystem, n))
        .cloned()
        .collect();
    let withheld = names.len() - kept.len();
    if withheld > 0 {
        tracing::info!(
            target: "4da::privacy",
            ecosystem,
            withheld,
            "Private package names withheld from the public registry"
        );
    }
    kept
}

/// [`may_look_up`] with the switch and the private set supplied.
fn allowed(enabled: bool, private: &PrivateNames, ecosystem: &str, name: &str) -> bool {
    if !enabled {
        return false;
    }
    let Some(eco) = Ecosystem::parse(ecosystem) else {
        return true;
    };
    if private.is_private(eco, name) {
        tracing::debug!(
            target: "4da::privacy",
            ecosystem = eco.osv_name(),
            "Private package name withheld from the public registry"
        );
        return false;
    }
    true
}

/// What the readiness record says about release lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LookupStatus {
    /// Settings → Privacy → Registry lookups is off.
    pub off: bool,
    /// Dependencies in the inventory never sent to a public registry because
    /// their names are private (distinct per ecosystem and name).
    pub private_withheld: u32,
}

/// The current [`LookupStatus`]. Reads the private set the last lookup
/// computed and never computes one itself (readiness is served on every
/// feed read; the registry sources compute it each fetch cycle). With the
/// switch off nothing is looked up at all, so the count does not matter.
pub(crate) fn lookup_status() -> LookupStatus {
    LookupStatus {
        off: !registry_lookups_enabled(),
        private_withheld: CACHE.lock().as_ref().map_or(0, |(_, n)| n.withheld),
    }
}

/// The last computed private set and when it was computed.
static CACHE: parking_lot::Mutex<Option<(Instant, Arc<PrivateNames>)>> =
    parking_lot::Mutex::new(None);

// ============================================================================
// The private set
// ============================================================================

/// The names that must never reach a public registry.
#[derive(Debug, Default, Clone)]
pub(crate) struct PrivateNames {
    /// `GOPRIVATE` / `GONOPROXY` / `GONOSUMDB` patterns.
    go_patterns: Vec<String>,
    /// npm scopes (`@corp`, lowercase) routed to a non-public registry.
    npm_scopes: HashSet<String>,
    /// The user's default npm registry is not the public one.
    npm_all: bool,
    /// The user's default Python index is not pypi.org.
    pypi_all: bool,
    /// Exact `(osv ecosystem, normalized name)` pairs.
    exact: HashSet<(&'static str, String)>,
    /// How many inventory dependencies the rules above withhold.
    pub(crate) withheld: u32,
}

impl PrivateNames {
    pub(crate) fn is_private(&self, eco: Ecosystem, name: &str) -> bool {
        let name = name.trim();
        if self.exact.contains(&(eco.osv_name(), normalize(eco, name))) {
            return true;
        }
        match eco {
            Ecosystem::Go => go_matches_any(&self.go_patterns, name),
            Ecosystem::Npm => {
                self.npm_all || scope_of(name).is_some_and(|scope| self.npm_scopes.contains(&scope))
            }
            Ecosystem::PyPI => self.pypi_all,
            _ => false,
        }
    }

    fn add(&mut self, eco: Ecosystem, name: &str) {
        let name = name.trim();
        if !name.is_empty() {
            self.exact.insert((eco.osv_name(), normalize(eco, name)));
        }
    }
}

/// A name as its registry compares names: PEP 503 for PyPI, `-` and `_`
/// alike for crates, lowercase for npm; Go module paths are case-sensitive.
fn normalize(eco: Ecosystem, name: &str) -> String {
    match eco {
        Ecosystem::PyPI => {
            let lower = name.to_lowercase();
            let mut out = String::with_capacity(lower.len());
            let mut last_sep = false;
            for c in lower.chars() {
                if matches!(c, '-' | '_' | '.') {
                    if !last_sep {
                        out.push('-');
                    }
                    last_sep = true;
                } else {
                    out.push(c);
                    last_sep = false;
                }
            }
            out
        }
        Ecosystem::Cargo => name.to_lowercase().replace('_', "-"),
        Ecosystem::Go => name.to_string(),
        _ => name.to_lowercase(),
    }
}

fn scope_of(name: &str) -> Option<String> {
    name.starts_with('@')
        .then(|| name.split('/').next().unwrap_or(name).to_ascii_lowercase())
}

/// The cached private set, recomputed every [`REFRESH`].
fn private_names() -> Arc<PrivateNames> {
    if let Some((at, names)) = CACHE.lock().as_ref() {
        if at.elapsed() < REFRESH {
            return names.clone();
        }
    }
    let names = Arc::new(compute_private_names());
    *CACHE.lock() = Some((Instant::now(), names.clone()));
    names
}

/// One project in the inventory: its folder and the dependencies stored for
/// it, as `(ecosystem alias, name)`.
#[derive(Debug, Default, Clone)]
pub(crate) struct ProjectDeps {
    pub root: PathBuf,
    pub deps: Vec<(String, String)>,
}

/// What the user-level configuration says, read once per computation.
#[derive(Debug, Default, Clone)]
pub(crate) struct UserConfig {
    /// Environment variables (only the ones the rules read).
    pub env: HashMap<String, String>,
    /// The `go env -w` file's text.
    pub go_env_file: Option<String>,
    /// `~/.npmrc`'s text.
    pub npmrc: Option<String>,
    /// The texts of the user's pip / uv configuration files.
    pub python_configs: Vec<String>,
}

const ENV_KEYS: &[&str] = &[
    "GOPRIVATE",
    "GONOPROXY",
    "GONOSUMDB",
    "npm_config_registry",
    "NPM_CONFIG_REGISTRY",
    "PIP_INDEX_URL",
    "PIP_EXTRA_INDEX_URL",
    "UV_INDEX_URL",
    "UV_DEFAULT_INDEX",
    "UV_INDEX",
    "UV_EXTRA_INDEX_URL",
];

fn compute_private_names() -> PrivateNames {
    let user = read_user_config();
    let projects = read_inventory_projects();
    build(&user, &projects)
}

/// Read the user-level configuration from this machine.
fn read_user_config() -> UserConfig {
    let env: HashMap<String, String> = ENV_KEYS
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_string(), v)))
        .collect();
    let go_env_path = match std::env::var("GOENV") {
        Ok(path) if path.trim() == "off" => None,
        Ok(path) if !path.trim().is_empty() => Some(PathBuf::from(path)),
        _ => dirs::config_dir().map(|d| d.join("go").join("env")),
    };
    let home = dirs::home_dir();
    let mut python_paths: Vec<PathBuf> = Vec::new();
    if let Ok(path) = std::env::var("PIP_CONFIG_FILE") {
        python_paths.push(PathBuf::from(path));
    }
    if let Some(config) = dirs::config_dir() {
        python_paths.push(config.join("pip").join("pip.ini"));
        python_paths.push(config.join("pip").join("pip.conf"));
        python_paths.push(config.join("uv").join("uv.toml"));
    }
    if let Some(home) = &home {
        python_paths.push(home.join("pip").join("pip.ini"));
        python_paths.push(home.join(".pip").join("pip.conf"));
        python_paths.push(home.join(".config").join("pip").join("pip.conf"));
        python_paths.push(home.join(".config").join("uv").join("uv.toml"));
    }
    python_paths.sort();
    python_paths.dedup();
    UserConfig {
        env,
        go_env_file: go_env_path.and_then(|p| std::fs::read_to_string(p).ok()),
        npmrc: home.and_then(|h| std::fs::read_to_string(h.join(".npmrc")).ok()),
        python_configs: python_paths
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .collect(),
    }
}

/// Every project folder in the inventory with its stored dependencies, read
/// on a connection of its own (this runs inside source fetches).
fn read_inventory_projects() -> Vec<ProjectDeps> {
    let Ok(conn) = crate::open_db_connection() else {
        return Vec::new();
    };
    let mut by_root: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for sql in [
        "SELECT project_path, ecosystem, package_name FROM user_dependencies",
        "SELECT project_path, language, package_name FROM project_dependencies",
        "SELECT project_path, ecosystem, package_name FROM dependency_instances",
    ] {
        let Ok(mut stmt) = conn.prepare(sql) else {
            continue;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) else {
            continue;
        };
        for (root, eco, name) in rows.flatten() {
            by_root.entry(root).or_default().push((eco, name));
        }
    }
    by_root
        .into_iter()
        .map(|(root, deps)| ProjectDeps {
            root: PathBuf::from(root),
            deps,
        })
        .collect()
}

/// The private set for this configuration and these projects (reads each
/// project's own files from disk).
pub(crate) fn build(user: &UserConfig, projects: &[ProjectDeps]) -> PrivateNames {
    let mut out = PrivateNames::default();

    // Go: the environment wins over the `go env -w` file, per variable.
    let go_file = parse_env_file(user.go_env_file.as_deref().unwrap_or(""));
    for key in ["GOPRIVATE", "GONOPROXY", "GONOSUMDB"] {
        let value = user
            .env
            .get(key)
            .cloned()
            .or_else(|| go_file.get(key).cloned())
            .unwrap_or_default();
        out.go_patterns.extend(
            value
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string),
        );
    }

    // npm, user level.
    if let Some(npmrc) = &user.npmrc {
        out.npm_scopes
            .extend(crate::ace::npm_local::private_registry_scopes(npmrc));
        out.npm_all |= npmrc_default_registry_is_private(npmrc);
    }
    for key in ["npm_config_registry", "NPM_CONFIG_REGISTRY"] {
        if let Some(url) = user.env.get(key) {
            out.npm_all |= !url.trim().is_empty() && !url_on(url, NPM_PUBLIC_HOSTS);
        }
    }

    // PyPI, user level.
    for key in [
        "PIP_INDEX_URL",
        "PIP_EXTRA_INDEX_URL",
        "UV_INDEX_URL",
        "UV_DEFAULT_INDEX",
        "UV_INDEX",
        "UV_EXTRA_INDEX_URL",
    ] {
        if let Some(value) = user.env.get(key) {
            out.pypi_all |= names_private_index(value);
        }
    }
    out.pypi_all |= user.python_configs.iter().any(|c| python_config_private(c));

    for project in projects {
        add_project(&mut out, project);
    }

    let mut withheld: HashSet<(&'static str, String)> = HashSet::new();
    for project in projects {
        for (eco, name) in &project.deps {
            let Some(eco) = registry_ecosystem(eco) else {
                continue;
            };
            if out.is_private(eco, name) {
                withheld.insert((eco.osv_name(), normalize(eco, name)));
            }
        }
    }
    out.withheld = u32::try_from(withheld.len()).unwrap_or(u32::MAX);
    out
}

/// The ecosystems 4DA looks releases up for.
fn registry_ecosystem(alias: &str) -> Option<Ecosystem> {
    Ecosystem::parse(alias).filter(|e| {
        matches!(
            e,
            Ecosystem::Npm | Ecosystem::Cargo | Ecosystem::PyPI | Ecosystem::Go
        )
    })
}

fn add_project(out: &mut PrivateNames, project: &ProjectDeps) {
    let root = project.root.as_path();
    let project_names = |eco: Ecosystem| {
        project
            .deps
            .iter()
            .filter(move |(e, _)| registry_ecosystem(e) == Some(eco))
            .map(|(_, n)| n.clone())
            .collect::<Vec<_>>()
    };

    // npm: scopes and a default registry from the project's `.npmrc` chain.
    let mut npm_project_private = false;
    for dir in root.ancestors().take(MAX_NPMRC_ANCESTORS) {
        if let Ok(npmrc) = std::fs::read_to_string(dir.join(".npmrc")) {
            out.npm_scopes
                .extend(crate::ace::npm_local::private_registry_scopes(&npmrc));
            npm_project_private |= npmrc_default_registry_is_private(&npmrc);
        }
    }
    if npm_project_private {
        for name in project_names(Ecosystem::Npm) {
            out.add(Ecosystem::Npm, &name);
        }
    }

    // Cargo.
    if let Ok(lock) = std::fs::read_to_string(root.join("Cargo.lock")) {
        for name in cargo_lock_private(&lock) {
            out.add(Ecosystem::Cargo, &name);
        }
    }
    if let Ok(manifest) = std::fs::read_to_string(root.join("Cargo.toml")) {
        for name in cargo_toml_private(&manifest) {
            out.add(Ecosystem::Cargo, &name);
        }
    }

    // Go.
    if let Ok(gomod) = std::fs::read_to_string(root.join("go.mod")) {
        for module in go_mod_private(&gomod) {
            out.add(Ecosystem::Go, &module);
        }
    }

    // PyPI.
    let mut pypi_project_private = false;
    for file in requirement_files(root) {
        if let Ok(text) = std::fs::read_to_string(&file) {
            let facts = requirements_private(&text);
            pypi_project_private |= facts.other_index;
            for name in facts.names {
                out.add(Ecosystem::PyPI, &name);
            }
        }
    }
    if let Ok(pyproject) = std::fs::read_to_string(root.join("pyproject.toml")) {
        let facts = pyproject_private(&pyproject);
        pypi_project_private |= facts.other_index;
        for name in facts.names {
            out.add(Ecosystem::PyPI, &name);
        }
    }
    for lock in ["uv.lock", "poetry.lock"] {
        if let Ok(text) = std::fs::read_to_string(root.join(lock)) {
            for name in python_lock_private(&text) {
                out.add(Ecosystem::PyPI, &name);
            }
        }
    }
    if pypi_project_private {
        for name in project_names(Ecosystem::PyPI) {
            out.add(Ecosystem::PyPI, &name);
        }
    }
}

// The file readers and Go's pattern matching live beside this module.
#[path = "registry_privacy_parsers.rs"]
mod parsers;
use parsers::*;

#[cfg(test)]
#[path = "registry_privacy_tests.rs"]
mod tests;

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `package-lock.json` / `npm-shrinkwrap.json` (lockfile v1, v2, v3) and
//! `pnpm-lock.yaml`.
//!
//! The package-lock reader used to skip every key with a second
//! `node_modules/` in it ("too deep"). Those are real installed copies: npm
//! nests a package wherever the hoisted copy does not satisfy a dependent's
//! range, and the nested one is routinely the OLD, vulnerable version
//! (measured 2026-10-10: 24% of npm advisory findings osv-scanner reports on
//! 19 repositories sat in nested copies). v1 lockfiles nest the same copies
//! under each entry's own `dependencies`, which were also never walked.

use std::path::PathBuf;

use serde_json::{Map, Value};

use super::{DepScope, LockFormat, LockedPackage, LockfileRead};
use crate::ace::dep_scope::DevScope;
use crate::ace::scanner::ProjectScanner;

/// A version an npm registry publishes (`1.2.3`, `0.0.1-security`), as
/// opposed to a git URL, tarball path, `file:` / `link:` spec or alias.
pub(super) fn is_registry_version(version: &str) -> bool {
    version.starts_with(|c: char| c.is_ascii_digit())
}

/// `name@version` split at the version's `@` (scoped names keep their own).
pub(super) fn split_name_at_version(spec: &str) -> Option<(&str, &str)> {
    let at = spec.get(1..)?.find('@')? + 1;
    Some((&spec[..at], &spec[at + 1..]))
}

/// The installed package name of a v2/v3 `packages` key: the segment after
/// its LAST `node_modules/`. `None` for the root (`""`) and for workspace
/// source directories (`packages/foo`), which are not installed copies.
fn install_name(key: &str) -> Option<&str> {
    key.rsplit_once("node_modules/").map(|(_, name)| name)
}

pub(super) fn read_package_lock(content: &str, format: LockFormat) -> Result<LockfileRead, String> {
    let lock: Value = serde_json::from_str(content).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut read = LockfileRead {
        path: PathBuf::new(),
        format,
        packages: Vec::new(),
        edges: ProjectScanner::parse_package_lock_edges(content),
        non_registry_entries: 0,
    };
    // npm's own dev verdict per copy (AD-046), read by the one implementation.
    let scope = DevScope::from_package_lock(content);
    if let Some(packages) = lock.get("packages").and_then(Value::as_object) {
        read_v2_packages(packages, &scope, &mut read);
    } else if let Some(deps) = lock.get("dependencies").and_then(Value::as_object) {
        read_v1_tree(deps, true, &scope, &mut read);
    } else if lock.get("lockfileVersion").is_none() {
        return Err("no `packages` or `dependencies` section".to_string());
    }
    Ok(read)
}

/// The copy's scope from npm's own `dev` marks (`devOptional` stays runtime).
fn scope_of(scope: &DevScope, installed_as: &str, version: &str) -> DepScope {
    match scope.label(installed_as, version) {
        "dev" => DepScope::Dev,
        "runtime" => DepScope::Runtime,
        _ => DepScope::Unknown,
    }
}

fn read_v2_packages(packages: &Map<String, Value>, scope: &DevScope, read: &mut LockfileRead) {
    for (key, entry) in packages {
        if key.is_empty() {
            continue; // the project itself
        }
        let Some(installed_as) = install_name(key) else {
            read.non_registry_entries += 1; // a workspace member's source dir
            continue;
        };
        let linked = entry.get("link").and_then(Value::as_bool) == Some(true);
        // An alias install (`"string-width-cjs": "npm:string-width@4.2.3"`)
        // records the real package in `name`.
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(installed_as);
        match entry.get("version").and_then(Value::as_str) {
            Some(version) if !linked && is_registry_version(version) => {
                let primary = key.len() == "node_modules/".len() + installed_as.len();
                read.packages.push(
                    LockedPackage::new(name, version)
                        .scoped(scope_of(scope, installed_as, version))
                        .primary(primary),
                );
            }
            _ => read.non_registry_entries += 1,
        }
    }
}

fn read_v1_tree(
    deps: &Map<String, Value>,
    top_level: bool,
    scope: &DevScope,
    read: &mut LockfileRead,
) {
    for (name, entry) in deps {
        if let Some(version) = entry.get("version").and_then(Value::as_str) {
            // v1 records an alias as `"version": "npm:real-name@1.2.3"`.
            let (real_name, real_version) = version
                .strip_prefix("npm:")
                .and_then(split_name_at_version)
                .unwrap_or((name.as_str(), version));
            if is_registry_version(real_version) {
                read.packages.push(
                    LockedPackage::new(real_name, real_version)
                        .scoped(scope_of(scope, name, version))
                        .primary(top_level),
                );
            } else {
                read.non_registry_entries += 1;
            }
        }
        if let Some(nested) = entry.get("dependencies").and_then(Value::as_object) {
            read_v1_tree(nested, false, scope, read);
        }
    }
}

/// pnpm-lock.yaml v5 / v6 / v9: the `packages:` identities, with each copy's
/// scope from the importer graph (AD-046) and the snapshot edges.
pub(super) fn read_pnpm_lock(content: &str) -> LockfileRead {
    let scope = DevScope::from_pnpm_lock(content);
    let mut seen = std::collections::HashSet::new();
    let packages = ProjectScanner::parse_pnpm_lock_yaml(content)
        .into_iter()
        .filter(|(name, version)| seen.insert((name.clone(), version.clone())))
        .map(|(name, version)| {
            let label = match scope.label(&name, &version) {
                "runtime" => DepScope::Runtime,
                "dev" => DepScope::Dev,
                _ => DepScope::Unknown,
            };
            LockedPackage::new(name, version).scoped(label)
        })
        .collect();
    LockfileRead {
        path: PathBuf::new(),
        format: LockFormat::Pnpm,
        packages,
        edges: ProjectScanner::parse_pnpm_lock_edges(content),
        non_registry_entries: 0,
    }
}

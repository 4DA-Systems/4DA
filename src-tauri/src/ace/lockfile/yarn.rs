// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `yarn.lock`, classic (v1) and berry (v2+).
//!
//! Berry writes `version: 1.2.3` (YAML) where classic writes
//! `version "1.2.3"`; the old reader matched only the classic spelling, so
//! every berry lockfile read as empty (measured 2026-10-10: jest's 26 berry
//! lockfiles, 2,093 packages in the root one alone). Berry also locks the
//! workspace itself (`0.0.0-use.local`) and patched copies of registry
//! packages, which are told apart by the `resolution:` protocol.

use std::collections::HashMap;
use std::path::PathBuf;

use super::npm::{is_registry_version, split_name_at_version};
use super::{LockFormat, LockedPackage, LockfileRead};

pub(super) fn read_yarn_lock(content: &str) -> Result<LockfileRead, String> {
    let berry = content.lines().any(|l| l.trim_end() == "__metadata:");
    let mut read = if berry {
        read_berry(content)
    } else {
        read_classic(content)
    };
    let entries = content
        .lines()
        .filter(|l| is_entry_key(l) && l.trim_end() != "__metadata:")
        .count();
    if entries > 0 && read.packages.is_empty() && read.non_registry_entries == 0 {
        return Err(format!(
            "{entries} entries but no version could be read (unrecognised yarn.lock layout)"
        ));
    }
    // One copy per (name, version); a version locked by two entries keeps
    // the descriptors of both.
    let mut index: HashMap<(String, String), usize> = HashMap::new();
    let mut packages: Vec<LockedPackage> = Vec::new();
    for pkg in std::mem::take(&mut read.packages) {
        match index.get(&(pkg.name.clone(), pkg.version.clone())) {
            Some(&i) => packages[i].specs.extend(pkg.specs),
            None => {
                index.insert((pkg.name.clone(), pkg.version.clone()), packages.len());
                packages.push(pkg);
            }
        }
    }
    read.packages = packages;
    Ok(read)
}

/// A descriptor in the one spelling both the lockfile and a manifest are
/// compared in: `name@range`, with berry's `npm:` protocol dropped from a
/// plain range (`lodash@npm:^4.17.21` -> `lodash@^4.17.21`). An alias
/// (`x-cjs@npm:x@^4`) keeps it: the range names another package.
pub(crate) fn normalize_descriptor(descriptor: &str) -> String {
    let descriptor = descriptor.trim().trim_matches('"');
    match split_name_at_version(descriptor) {
        Some((name, range)) => match range.strip_prefix("npm:") {
            Some(plain) if !plain.contains('@') => format!("{name}@{plain}"),
            _ => descriptor.to_string(),
        },
        None => descriptor.to_string(),
    }
}

/// Every descriptor of an entry key: `"a@^1, a@^1.2":` -> `[a@^1, a@^1.2]`.
fn descriptors(line: &str) -> Vec<String> {
    let key = line.trim_end().trim_end_matches(':');
    key.split(',').map(normalize_descriptor).collect()
}

/// yarn.lock records no install position: a copy is placed only when a
/// manifest's declared range matches one of its descriptors.
fn unplaced(pkg: LockedPackage, specs: Vec<String>) -> LockedPackage {
    let mut pkg = pkg.top_level(false);
    pkg.specs = specs;
    pkg
}

fn empty(format: LockFormat) -> LockfileRead {
    LockfileRead {
        path: PathBuf::new(),
        format,
        packages: Vec::new(),
        edges: Vec::new(),
        non_registry_entries: 0,
    }
}

/// A top-level entry line: no indent, not a comment, ends with `:`.
fn is_entry_key(line: &str) -> bool {
    !line.is_empty() && !line.starts_with([' ', '\t', '#']) && line.trim_end().ends_with(':')
}

/// The first descriptor of an entry key: `"a@^1, a@^1.2":` -> `a@^1`.
fn first_descriptor(line: &str) -> String {
    let key = line.trim_end().trim_end_matches(':');
    let first = key.split(',').next().unwrap_or(key);
    first.trim().trim_matches('"').to_string()
}

/// Classic: the name from the key's first descriptor (alias-aware), the
/// version from `version "x"`.
fn read_classic(content: &str) -> LockfileRead {
    let mut read = empty(LockFormat::YarnClassic);
    let mut current: Option<(String, bool)> = None;
    let mut specs: Vec<String> = Vec::new();
    for line in content.lines() {
        if is_entry_key(line) {
            current = classic_name(&first_descriptor(line));
            specs = descriptors(line);
            if current.is_none() {
                read.non_registry_entries += 1;
            }
            continue;
        }
        let Some(rest) = line.trim().strip_prefix("version ") else {
            continue;
        };
        if let Some((name, registry)) = current.take() {
            let version = rest.trim().trim_matches('"');
            if registry && is_registry_version(version) {
                read.packages.push(unplaced(
                    LockedPackage::new(name, version),
                    std::mem::take(&mut specs),
                ));
            } else {
                read.non_registry_entries += 1;
            }
        }
    }
    read
}

/// `(package name, whether the range names a registry release)` for a
/// classic descriptor. `x-cjs@npm:x@^4` is the package `x`.
fn classic_name(descriptor: &str) -> Option<(String, bool)> {
    let (name, range) = split_name_at_version(descriptor)?;
    if let Some(target) = range.strip_prefix("npm:") {
        return match split_name_at_version(target) {
            Some((real, _)) => Some((real.to_string(), true)),
            None => Some((target.to_string(), true)), // `npm:x` = latest x
        };
    }
    const NON_REGISTRY: &[&str] = &[
        "git",
        "github:",
        "gitlab:",
        "bitbucket:",
        "file:",
        "link:",
        "http:",
        "https:",
        "portal:",
    ];
    let registry = !NON_REGISTRY.iter().any(|p| range.starts_with(p));
    Some((name.to_string(), registry))
}

/// Berry: one entry = key + 2-space fields; `resolution:` decides what was
/// installed and from where.
fn read_berry(content: &str) -> LockfileRead {
    let mut read = empty(LockFormat::YarnBerry);
    let mut version: Option<String> = None;
    let mut resolution: Option<String> = None;
    let mut specs: Vec<String> = Vec::new();
    let mut in_entry = false;
    let mut flush =
        |version: &mut Option<String>, resolution: &mut Option<String>, specs: &mut Vec<String>| {
            if let (Some(v), Some(r)) = (version.take(), resolution.take()) {
                match berry_package(&r, &v) {
                    Some(pkg) => read.packages.push(unplaced(pkg, std::mem::take(specs))),
                    None => read.non_registry_entries += 1,
                }
            }
        };
    for line in content.lines() {
        if is_entry_key(line) {
            flush(&mut version, &mut resolution, &mut specs);
            in_entry = line.trim_end() != "__metadata:";
            specs = descriptors(line);
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if !in_entry || indent != 2 {
            continue;
        }
        let field = line.trim();
        if let Some(v) = field.strip_prefix("version:") {
            version = Some(v.trim().trim_matches('"').to_string());
        } else if let Some(r) = field.strip_prefix("resolution:") {
            resolution = Some(r.trim().trim_matches('"').to_string());
        }
    }
    flush(&mut version, &mut resolution, &mut specs);
    read
}

/// A registry copy from a berry `resolution` (`name@npm:1.2.3`, or a
/// `patch:` of one); `None` for workspace, link, portal, file, git and
/// tarball resolutions.
fn berry_package(resolution: &str, version: &str) -> Option<LockedPackage> {
    let (name, locator) = split_name_at_version(resolution)?;
    let registry = locator.starts_with("npm:")
        || (locator.starts_with("patch:")
            && (locator.contains("@npm%3A") || locator.contains("@npm:")));
    (registry && is_registry_version(version)).then(|| LockedPackage::new(name, version))
}

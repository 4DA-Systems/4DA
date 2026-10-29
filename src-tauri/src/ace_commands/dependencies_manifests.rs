// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! What each directory's MANIFEST declares, per ecosystem: the names that make
//! a locked copy direct, and the rows the lockfile prune must keep. Split out
//! of `dependencies.rs` (file-size gate).

use std::path::{Path, PathBuf};

use crate::ace::lockfile::{LockFormat, LockfileRead};

/// The names the directory's manifest declares for `ecosystem` — what makes
/// a locked copy DIRECT. A requirements file's pins and a Pipfile's entries
/// are declarations themselves.
pub(super) fn direct_names(
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    ecosystem: &str,
    reads: &[&LockfileRead],
) -> Vec<String> {
    match ecosystem {
        "javascript" => {
            let mut names = read_package_json_deps(scanner, dir);
            names.extend(workspace_member_deps(scanner, dir));
            names
        }
        "python" => {
            let mut names = read_pyproject_deps(scanner, dir);
            names.extend(read_pipfile_deps(dir));
            names.extend(
                reads
                    .iter()
                    .filter(|r| r.format == LockFormat::Requirements)
                    .flat_map(|r| r.packages.iter().map(|p| p.name.clone())),
            );
            names
        }
        "go" => read_go_mod_deps(scanner, dir),
        "ruby" => read_gemfile_deps(dir),
        "php" => read_composer_json_deps(dir),
        _ => Vec::new(),
    }
}

/// A JavaScript workspace's declarations split by owner: the root
/// manifest's own names, and each member's with the keys a lockfile writes
/// for its slot (its `/`-separated directory relative to `dir`, and its
/// package name). `(vec![], vec![])` for a single-root project or another
/// ecosystem.
pub(super) fn workspace_declarations(
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &Path,
    ecosystem: &str,
) -> (Vec<String>, Vec<super::store::MemberDeclarations>) {
    if ecosystem != "javascript" {
        return (Vec::new(), Vec::new());
    }
    let members = crate::ace::npm_local::workspace_member_dirs(dir);
    if members.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let declared = members
        .iter()
        .map(|m| {
            let mut keys: Vec<String> = Vec::new();
            if let Ok(rel) = m.strip_prefix(dir) {
                let rel: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                keys.push(rel.join("/"));
            }
            keys.extend(crate::ace::npm_local::package_name(m));
            super::store::MemberDeclarations {
                keys,
                names: read_package_json_deps(scanner, &m.to_path_buf()),
            }
        })
        .collect();
    (
        read_package_json_deps(scanner, &dir.to_path_buf()),
        declared,
    )
}

/// The names the directory's manifest declares ONLY for development, for the
/// ecosystems whose lockfiles record no scope of their own. Python: the
/// pyproject's PEP 735 groups and uv / PDM / Poetry dev tables, minus every
/// runtime requirement (A-4). Elsewhere the lockfile carries the verdict
/// (npm/pnpm/bun graphs, Cargo roots, Pipfile.lock `develop`).
pub(super) fn dev_only_names(dir: &Path, ecosystem: &str) -> Vec<String> {
    if ecosystem != "python" {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(dir.join("pyproject.toml")) else {
        return Vec::new();
    };
    let runtime: Vec<String> = crate::ace::scanner::parse_pyproject_dependencies(&content)
        .iter()
        .map(|n| crate::osv::matching::pep503_name(n))
        .collect();
    crate::ace::pyproject_dev::parse_pyproject_dev_dependencies(&content)
        .into_iter()
        .filter(|n| !runtime.contains(&crate::osv::matching::pep503_name(n)))
        .collect()
}

/// Every `name@range` the directory's manifests declare for `ecosystem`, in
/// [`crate::ace::lockfile::normalize_descriptor`]'s spelling — how a
/// yarn.lock copy, which records no install position, is matched to the
/// declaration it resolves. JavaScript only (the root `package.json` and its
/// workspace members); empty elsewhere.
pub(super) fn declared_specs(dir: &Path, ecosystem: &str) -> Vec<String> {
    if ecosystem != "javascript" {
        return Vec::new();
    }
    std::iter::once(dir.to_path_buf())
        .chain(crate::ace::npm_local::workspace_member_dirs(dir))
        .flat_map(|d| package_json_specs(&d))
        .collect()
}

fn package_json_specs(dir: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(dir.join("package.json")) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };
    ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .filter_map(|field| parsed.get(*field).and_then(|v| v.as_object()))
        .flat_map(|map| map.iter())
        .filter_map(|(name, range)| {
            let range = range.as_str()?;
            Some(crate::ace::lockfile::normalize_descriptor(&format!(
                "{name}@{range}"
            )))
        })
        .collect()
}

/// Rows the prune must keep beyond the lockfiles' packages: the go.mod
/// `go`/`toolchain` directives become synthetic "stdlib" / "toolchain" rows
/// (store_go_directive_dependencies) — keep them, or every scan would delete
/// and re-insert them.
pub(super) fn keep_names(dir: &Path, ecosystem: &str) -> Vec<String> {
    if ecosystem != "go" {
        return Vec::new();
    }
    std::fs::read_to_string(dir.join("go.mod"))
        .map(|go_mod| {
            crate::ace::scanner::ProjectScanner::parse_go_directives(&go_mod)
                .into_iter()
                .map(|(name, _)| name)
                .collect()
        })
        .unwrap_or_default()
}

/// Shared: the package names a `Pipfile` declares (`[packages]` and
/// `[dev-packages]`), for Pipfile.lock's direct set.
fn read_pipfile_deps(dir: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(dir.join("Pipfile")) else {
        return Vec::new();
    };
    let mut in_packages = false;
    let mut names = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_packages = trimmed == "[packages]" || trimmed == "[dev-packages]";
            continue;
        }
        if !in_packages || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            let name = name.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn read_composer_json_deps(dir: &PathBuf) -> Vec<String> {
    let path = dir.join("composer.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };
    parsed
        .get("require")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

/// Shared: read direct deps from package.json for lockfile processing.
fn read_package_json_deps(
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
) -> Vec<String> {
    if let Ok(pkg_content) = std::fs::read_to_string(dir.join("package.json")) {
        let mut signal = crate::ace::scanner::ProjectSignal {
            manifest_type: crate::ace::scanner::ManifestType::PackageJson,
            manifest_path: dir.join("package.json"),
            project_name: None,
            languages: vec!["javascript".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            indirect_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };
        scanner.parse_package_json(&pkg_content, &mut signal);
        let mut all = signal.dependencies;
        all.extend(signal.dev_dependencies);
        all
    } else {
        Vec::new()
    }
}

/// What the members of the workspace rooted at `dir` declare, runtime and
/// dev: the workspace's one lockfile (here, at the root) resolves them, so
/// they are as direct as the root's own. Without them a dependency only a
/// member declared was stored transitive, and the parent walk
/// (`osv::parent_hint`, which stops at the first direct copy) found no lever
/// for it. The members' own packages are local code, never dependencies.
/// Empty for a single-root project.
fn workspace_member_deps(scanner: &crate::ace::scanner::ProjectScanner, dir: &Path) -> Vec<String> {
    let members = crate::ace::npm_local::workspace_member_dirs(dir);
    if members.is_empty() {
        return Vec::new();
    }
    let local: Vec<String> = members
        .iter()
        .map(PathBuf::as_path)
        .chain(std::iter::once(dir))
        .filter_map(crate::ace::npm_local::package_name)
        .collect();
    members
        .iter()
        .flat_map(|m| read_package_json_deps(scanner, m))
        .filter(|name| !local.contains(name))
        .collect()
}

/// Shared: read direct deps from pyproject.toml for poetry.lock processing.
fn read_pyproject_deps(
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
) -> Vec<String> {
    if let Ok(content) = std::fs::read_to_string(dir.join("pyproject.toml")) {
        let mut signal = crate::ace::scanner::ProjectSignal {
            manifest_type: crate::ace::scanner::ManifestType::PyprojectToml,
            manifest_path: dir.join("pyproject.toml"),
            project_name: None,
            languages: vec!["python".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            indirect_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };
        scanner.parse_pyproject_toml(&content, &mut signal);
        let mut all = signal.dependencies;
        all.extend(signal.dev_dependencies);
        all
    } else {
        Vec::new()
    }
}

/// Shared: read direct deps from go.mod for go.sum processing.
fn read_go_mod_deps(scanner: &crate::ace::scanner::ProjectScanner, dir: &PathBuf) -> Vec<String> {
    if let Ok(content) = std::fs::read_to_string(dir.join("go.mod")) {
        let mut signal = crate::ace::scanner::ProjectSignal {
            manifest_type: crate::ace::scanner::ManifestType::GoMod,
            manifest_path: dir.join("go.mod"),
            project_name: None,
            languages: vec!["go".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            indirect_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };
        scanner.parse_go_mod(&content, &mut signal);
        let mut all = signal.dependencies;
        all.extend(signal.dev_dependencies);
        all
    } else {
        Vec::new()
    }
}

/// Shared: read direct deps from Gemfile for Gemfile.lock processing.
/// Gemfile uses a simple DSL — we extract gem names from `gem 'name'` lines.
fn read_gemfile_deps(dir: &PathBuf) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(dir.join("Gemfile")) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("gem ") {
            // gem 'name', '~> 1.0'  or  gem "name"
            let rest = rest.trim();
            let quote = if rest.starts_with('\'') {
                '\''
            } else if rest.starts_with('"') {
                '"'
            } else {
                continue;
            };
            if let Some(end) = rest[1..].find(quote) {
                let name = &rest[1..=end];
                if !name.is_empty() {
                    deps.push(name.to_string());
                }
            }
        }
    }
    deps
}

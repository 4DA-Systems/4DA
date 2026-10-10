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
        "javascript" => read_package_json_deps(scanner, dir),
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

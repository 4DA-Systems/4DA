// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE dependency storage: direct and transitive dependency discovery from lockfiles.

use std::path::PathBuf;

use tracing::info;

use crate::db::Database;
use crate::get_ace_engine;

/// Store discovered direct dependencies from ACE into user_dependencies table.
pub(super) fn store_direct_dependencies(db: &Database) {
    if let Ok(ace) = get_ace_engine() {
        if let Ok(tech) = ace.get_detected_tech() {
            if let Ok(conn) = crate::open_db_connection() {
                if let Ok(deps) = crate::temporal::get_all_dependencies(&conn) {
                    for dep in &deps {
                        let ecosystem = &dep.language;
                        db.store_dependency(
                            &dep.project_path,
                            &dep.package_name,
                            dep.version.as_deref(),
                            ecosystem,
                            dep.is_dev,
                            None, // license extracted during manifest parsing
                        )
                        .ok();
                    }
                    if !deps.is_empty() {
                        info!(target: "4da::ace", count = deps.len(), "Stored dependencies in user_dependencies table");
                    }
                }
            }
            drop(tech);
        }
    }
}

/// Parse lockfiles for transitive dependency discovery and store in the database.
pub(super) fn store_lockfile_dependencies(db: &Database, scan_paths: &[PathBuf]) {
    let scanner = crate::ace::scanner::ProjectScanner::new();
    let mut lockfile_count = 0u32;

    for path in scan_paths {
        if !path.exists() || !path.is_dir() {
            continue;
        }
        let mut dirs_to_visit = vec![(path.clone(), 0u8)];
        while let Some((dir, depth)) = dirs_to_visit.pop() {
            if depth > 5 {
                continue;
            }
            let project_path = dir.to_string_lossy().to_string();

            lockfile_count += process_cargo_lock(db, &scanner, &dir, &project_path);
            lockfile_count += process_package_lock(db, &scanner, &dir, &project_path);
            lockfile_count += process_pnpm_lock(db, &scanner, &dir, &project_path);
            lockfile_count += process_yarn_lock(db, &scanner, &dir, &project_path);
            lockfile_count += process_poetry_lock(db, &scanner, &dir, &project_path);
            lockfile_count += process_go_sum(db, &scanner, &dir, &project_path);
            lockfile_count += process_gemfile_lock(db, &scanner, &dir, &project_path);
            lockfile_count += process_composer_lock(db, &dir, &project_path);

            // Recurse into subdirectories (skip common non-project dirs)
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                            if !matches!(
                                name,
                                "node_modules"
                                    | "target"
                                    | ".git"
                                    | "dist"
                                    | "build"
                                    | ".next"
                                    | "__pycache__"
                                    | ".venv"
                                    | "venv"
                                    | "vendor"
                                    | ".cargo"
                            ) {
                                dirs_to_visit.push((entry_path, depth + 1));
                            }
                        }
                    }
                }
            }
        }
    }

    if lockfile_count > 0 {
        info!(target: "4da::ace", count = lockfile_count, "Stored transitive dependencies from lockfiles");
    }
}

/// Process a Cargo.lock file, storing transitive deps and updating direct dep versions.
/// Returns the number of transitive dependencies stored.
fn process_cargo_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let cargo_lock = dir.join("Cargo.lock");
    if !cargo_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&cargo_lock) else {
        return 0;
    };

    let direct_deps: Vec<String> =
        if let Ok(toml_content) = std::fs::read_to_string(dir.join("Cargo.toml")) {
            let mut signal = crate::ace::scanner::ProjectSignal {
                manifest_type: crate::ace::scanner::ManifestType::CargoToml,
                manifest_path: dir.join("Cargo.toml"),
                project_name: None,
                languages: vec!["rust".to_string()],
                frameworks: Vec::new(),
                dependencies: Vec::new(),
                dev_dependencies: Vec::new(),
                target_dependencies: Vec::new(),
                detected_at: String::new(),
                project_license: None,
                project_relevance: 1.0, // lockfile processing uses default; relevance applied at manifest scan
            };
            scanner.parse_cargo_toml(&toml_content, &mut signal);
            let mut all = signal.dependencies;
            all.extend(signal.dev_dependencies);
            all
        } else {
            Vec::new()
        };

    // Capture the parent->child graph for reachability (Step 1, silent).
    let edges = crate::ace::scanner::ProjectScanner::parse_cargo_lock_edges(&content);
    db.store_dependency_edges(project_path, "rust", &edges).ok();

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_cargo_lock(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "rust",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "rust",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

/// Process a package-lock.json file, storing transitive deps and updating direct dep versions.
/// Returns the number of transitive dependencies stored.
fn process_package_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let pkg_lock = dir.join("package-lock.json");
    if !pkg_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&pkg_lock) else {
        return 0;
    };

    let direct_deps = read_package_json_deps(scanner, dir);

    // Capture the parent->child graph for reachability (Step 1, silent).
    let edges = crate::ace::scanner::ProjectScanner::parse_package_lock_edges(&content);
    db.store_dependency_edges(project_path, "javascript", &edges)
        .ok();

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_package_lock_json(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

/// Process a pnpm-lock.yaml file, storing transitive deps and updating direct dep versions.
fn process_pnpm_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let pnpm_lock = dir.join("pnpm-lock.yaml");
    if !pnpm_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&pnpm_lock) else {
        return 0;
    };

    let direct_deps = read_package_json_deps(scanner, dir);

    // Capture the parent->child graph for reachability (Step 1, silent).
    let edges = crate::ace::scanner::ProjectScanner::parse_pnpm_lock_edges(&content);
    db.store_dependency_edges(project_path, "javascript", &edges)
        .ok();

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_pnpm_lock_yaml(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

/// Process a yarn.lock file, storing transitive deps and updating direct dep versions.
fn process_yarn_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let yarn_lock = dir.join("yarn.lock");
    if !yarn_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&yarn_lock) else {
        return 0;
    };

    let direct_deps = read_package_json_deps(scanner, dir);

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_yarn_lock(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "javascript",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

/// Process a poetry.lock file, storing transitive deps and updating direct dep versions.
fn process_poetry_lock(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let poetry_lock = dir.join("poetry.lock");
    if !poetry_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&poetry_lock) else {
        return 0;
    };

    let direct_deps = read_pyproject_deps(scanner, dir);

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_poetry_lock(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d.eq_ignore_ascii_case(name)) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "python",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "python",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

/// Process a go.sum file, storing transitive deps and updating direct dep versions.
fn process_go_sum(
    db: &Database,
    scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let go_sum = dir.join("go.sum");
    if !go_sum.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&go_sum) else {
        return 0;
    };

    let direct_deps = read_go_mod_deps(scanner, dir);

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_go_sum(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(project_path, name, Some(version.as_str()), "go", false)
                .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "go",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

/// Process a Gemfile.lock, storing transitive deps and updating direct dep versions.
fn process_gemfile_lock(
    db: &Database,
    _scanner: &crate::ace::scanner::ProjectScanner,
    dir: &PathBuf,
    project_path: &str,
) -> u32 {
    let gemfile_lock = dir.join("Gemfile.lock");
    if !gemfile_lock.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&gemfile_lock) else {
        return 0;
    };

    let direct_deps = read_gemfile_deps(dir);

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_gemfile_lock(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "ruby",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "ruby",
                false,
                None,
            )
            .ok();
        }
    }
    count
}

fn process_composer_lock(db: &Database, dir: &PathBuf, project_path: &str) -> u32 {
    let lockfile = dir.join("composer.lock");
    if !lockfile.exists() {
        return 0;
    }
    let Ok(content) = std::fs::read_to_string(&lockfile) else {
        return 0;
    };

    let direct_deps = read_composer_json_deps(dir);

    let mut count = 0u32;
    let packages = crate::ace::scanner::ProjectScanner::parse_composer_lock(&content);
    for (name, version) in &packages {
        if direct_deps.is_empty() || !direct_deps.iter().any(|d| d == name) {
            db.store_transitive_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "php",
                false,
            )
            .ok();
            count += 1;
        } else {
            db.store_dependency(
                project_path,
                name,
                Some(version.as_str()),
                "php",
                false,
                None,
            )
            .ok();
        }
    }
    count
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

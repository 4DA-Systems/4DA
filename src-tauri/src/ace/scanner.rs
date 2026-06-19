// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Project Manifest Scanner
//!
//! Scans directories for project manifests and extracts technology stack.
//! Supports: package.json, Cargo.toml, pyproject.toml, go.mod, and more.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::error::Result;

/// True when `path` is a cloud-only placeholder — a OneDrive / Dropbox
/// "online-only" file whose CONTENT is not on local disk. Reading such a
/// file forces a download; during an unattended onboarding scan over a
/// KFM-redirected Documents folder that can pull gigabytes on a metered
/// connection without consent — and the app gets blamed for the bill. A
/// dehydrated file contributes no signal worth a forced download, so we
/// skip it. Uses `symlink_metadata`, which reads the placeholder's reparse
/// attributes WITHOUT touching content (no hydration), so the check is free.
#[cfg(windows)]
pub(crate) fn is_cloud_placeholder(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    // Win32 file attributes for cloud-on-demand / HSM dehydration.
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    match fs::symlink_metadata(path) {
        Ok(m) => {
            m.file_attributes()
                & (FILE_ATTRIBUTE_OFFLINE
                    | FILE_ATTRIBUTE_RECALL_ON_OPEN
                    | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
                != 0
        }
        Err(_) => false,
    }
}

/// No cloud-placeholder concept off Windows.
#[cfg(not(windows))]
pub(crate) fn is_cloud_placeholder(_path: &Path) -> bool {
    false
}

/// Project Scanner - detects projects and their tech stacks
pub struct ProjectScanner {
    /// Maximum depth to recurse into directories
    max_depth: usize,
    /// Directories to skip
    skip_dirs: HashSet<String>,
}

/// Types of manifests we can parse
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ManifestType {
    CargoToml,
    PackageJson,
    PyprojectToml,
    RequirementsTxt,
    GoMod,
    ComposerJson,
    Gemfile,
    PomXml,
    BuildGradle,
    CMakeLists,
    Csproj,
    PubspecYaml,
}

impl ManifestType {
    fn filename(&self) -> &'static str {
        match self {
            ManifestType::CargoToml => "Cargo.toml",
            ManifestType::PackageJson => "package.json",
            ManifestType::PyprojectToml => "pyproject.toml",
            ManifestType::RequirementsTxt => "requirements.txt",
            ManifestType::GoMod => "go.mod",
            ManifestType::ComposerJson => "composer.json",
            ManifestType::Gemfile => "Gemfile",
            ManifestType::PomXml => "pom.xml",
            ManifestType::BuildGradle => "build.gradle",
            ManifestType::CMakeLists => "CMakeLists.txt",
            ManifestType::Csproj => "*.csproj",
            ManifestType::PubspecYaml => "pubspec.yaml",
        }
    }

    pub(crate) fn language(&self) -> &'static str {
        match self {
            ManifestType::CargoToml => "rust",
            ManifestType::PackageJson => "javascript",
            ManifestType::PyprojectToml | ManifestType::RequirementsTxt => "python",
            ManifestType::GoMod => "go",
            ManifestType::ComposerJson => "php",
            ManifestType::Gemfile => "ruby",
            ManifestType::PomXml | ManifestType::BuildGradle => "java",
            ManifestType::CMakeLists => "cpp",
            ManifestType::Csproj => "csharp",
            ManifestType::PubspecYaml => "dart",
        }
    }
}

/// Signal from scanning a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSignal {
    pub manifest_type: ManifestType,
    pub manifest_path: PathBuf,
    pub project_name: Option<String>,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub dependencies: Vec<String>,
    pub dev_dependencies: Vec<String>,
    /// Platform-gated DIRECT deps from `[target.'cfg(...)'.dependencies]`:
    /// (package name, target spec e.g. "cfg(windows)"). Used to flag advisories
    /// that aren't relevant on the user's build platform.
    pub target_dependencies: Vec<(String, String)>,
    pub detected_at: String,
    /// Project-level license extracted from the manifest (e.g., "MIT", "Apache-2.0").
    pub project_license: Option<String>,
    /// Relevance score (0.0..1.0) based on path patterns and git recency.
    /// Example/demo/test directories get 0.1x; stale repos decay over time.
    pub project_relevance: f32,
}

// Framework signatures for the P2 ecosystems — substring match on the dependency id
// (composer "vendor/pkg", maven "groupId:artifactId", NuGet id, pub package). Only actual
// web/app frameworks; runtimes/ORMs are surfaced via is_notable_dependency() in ace/context.rs.
const PHP_FRAMEWORKS: &[(&str, &str)] = &[
    ("laravel/", "laravel"),
    ("symfony/", "symfony"),
    ("slim/slim", "slim"),
    ("cakephp/", "cakephp"),
    ("laminas/", "laminas"),
    ("yiisoft/", "yii"),
];
const RUBY_FRAMEWORKS: &[(&str, &str)] = &[
    ("rails", "rails"),
    ("sinatra", "sinatra"),
    ("hanami", "hanami"),
    ("roda", "roda"),
];
const JVM_FRAMEWORKS: &[(&str, &str)] = &[
    ("org.springframework", "spring"),
    ("io.quarkus", "quarkus"),
    ("io.micronaut", "micronaut"),
    ("play.", "play"),
    ("io.vertx", "vertx"),
    ("io.ktor", "ktor"),
];
const DOTNET_FRAMEWORKS: &[(&str, &str)] = &[
    ("microsoft.aspnetcore", "aspnet"),
    ("microsoft.maui", "maui"),
    ("blazor", "blazor"),
];
const DART_FRAMEWORKS: &[(&str, &str)] = &[("flutter", "flutter")];

impl ProjectScanner {
    pub fn new() -> Self {
        let mut skip_dirs = HashSet::new();
        // Common directories to skip
        skip_dirs.insert("node_modules".to_string());
        skip_dirs.insert("target".to_string());
        skip_dirs.insert(".git".to_string());
        skip_dirs.insert("dist".to_string());
        skip_dirs.insert("build".to_string());
        skip_dirs.insert(".next".to_string());
        skip_dirs.insert("__pycache__".to_string());
        skip_dirs.insert(".venv".to_string());
        skip_dirs.insert("venv".to_string());
        skip_dirs.insert("vendor".to_string());
        skip_dirs.insert(".cargo".to_string());
        skip_dirs.insert("pkg".to_string());
        // Sensitive directories — prevent scanning credentials, keys, secrets
        skip_dirs.insert(".ssh".to_string());
        skip_dirs.insert(".aws".to_string());
        skip_dirs.insert(".gnupg".to_string());
        skip_dirs.insert(".gpg".to_string());
        skip_dirs.insert(".docker".to_string());
        skip_dirs.insert(".kube".to_string());
        skip_dirs.insert(".terraform".to_string());
        skip_dirs.insert(".vault".to_string());
        skip_dirs.insert(".password-store".to_string());
        skip_dirs.insert(".env".to_string());
        // Note: .config/gcloud and .local/share/keyrings are multi-segment paths
        // handled by the component check below (skip_dirs matches single dir names).
        // We add the leaf segments so they're caught when traversed into:
        skip_dirs.insert("gcloud".to_string());
        skip_dirs.insert("keyrings".to_string());
        // macOS system metadata directories — waste I/O if scanned
        skip_dirs.insert(".Spotlight-V100".to_string());
        skip_dirs.insert(".fseventsd".to_string());
        skip_dirs.insert(".Trash".to_string());
        skip_dirs.insert(".TemporaryItems".to_string());
        skip_dirs.insert(".DocumentRevisions-V100".to_string());

        Self {
            max_depth: 8, // Deep enough for monorepos and nested workspaces
            skip_dirs,
        }
    }

    /// Maximum signals to collect (prevents OOM on huge repos).
    /// 2000 signals covers large codebases with 100+ packages/services.
    const MAX_SIGNALS: usize = 2000;

    /// Maximum path length to process (Windows MAX_PATH guard)
    #[cfg(target_os = "windows")]
    const MAX_PATH_LEN: usize = 260;
    #[cfg(not(target_os = "windows"))]
    const MAX_PATH_LEN: usize = 4096;

    /// Check if a path contains known non-project subdirectory patterns that
    /// should be excluded from scanning. These are multi-segment path patterns
    /// that can't be caught by the single-name `skip_dirs` check.
    fn is_excluded_path(path: &Path) -> bool {
        // Normalize to forward slashes for consistent matching on all platforms
        let path_str = path.to_string_lossy();

        // Patterns that indicate this is NOT a real project directory:
        // - .claude/worktrees/ — Claude Code agent worktrees (temporary repo copies)
        // - .git/worktrees/   — git's internal worktree metadata
        for pattern in &[
            ".claude/worktrees/",
            ".claude\\worktrees\\",
            ".git/worktrees/",
            ".git\\worktrees\\",
        ] {
            if path_str.contains(pattern) {
                return true;
            }
        }

        false
    }

    /// Scan a directory for project manifests
    pub fn scan_directory(&self, path: &Path) -> Result<Vec<ProjectSignal>> {
        let mut signals = Vec::new();
        let mut visited = HashSet::new();
        self.scan_recursive(path, 0, &mut signals, &mut visited)?;
        Ok(signals)
    }

    fn scan_recursive(
        &self,
        path: &Path,
        depth: usize,
        signals: &mut Vec<ProjectSignal>,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<()> {
        // Bounds check: depth and total signals
        if depth > self.max_depth || signals.len() >= Self::MAX_SIGNALS {
            if signals.len() >= Self::MAX_SIGNALS {
                tracing::warn!(
                    target: "4da::ace",
                    limit = Self::MAX_SIGNALS,
                    depth,
                    "Project scan signal limit reached — large monorepos may have incomplete detection"
                );
            }
            return Ok(());
        }

        if !path.is_dir() {
            return Ok(());
        }

        // Symlink cycle detection: resolve to canonical path and check if already visited
        let canonical = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return Ok(()), // Can't resolve path, skip
        };
        if !visited.insert(canonical.clone()) {
            warn!(target: "ace::scanner", path = %path.display(), "Symlink cycle detected, skipping");
            return Ok(());
        }

        // MAX_PATH guard (primarily for Windows)
        if path.as_os_str().len() > Self::MAX_PATH_LEN {
            warn!(target: "ace::scanner", path_len = path.as_os_str().len(), "Path exceeds max length, skipping");
            return Ok(());
        }

        // Check if this directory should be skipped
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            if self.skip_dirs.contains(dir_name) {
                return Ok(());
            }
        }

        // Skip paths that contain known non-project subdirectory patterns.
        // These are multi-segment paths that can't be caught by the single-name
        // skip_dirs check above:
        // - .claude/worktrees/  — orphaned Claude Code agent worktrees (copies of the same repo)
        // - .git/worktrees/     — git's own worktree metadata
        if Self::is_excluded_path(path) {
            return Ok(());
        }

        // Check for manifests in this directory
        self.check_manifests(path, signals)?;

        // Recurse into subdirectories
        let entries = fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;

        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Don't propagate errors from subdirectories - just skip them
                if let Err(e) = self.scan_recursive(&entry_path, depth + 1, signals, visited) {
                    tracing::warn!("Recursive scan failed: {e}");
                }
            }
        }

        Ok(())
    }

    fn check_manifests(&self, dir: &Path, signals: &mut Vec<ProjectSignal>) -> Result<()> {
        // Track where new signals start so we can merge imports into all of them
        let signals_start = signals.len();

        // Check each manifest type
        let manifest_types = [
            ManifestType::CargoToml,
            ManifestType::PackageJson,
            ManifestType::PyprojectToml,
            ManifestType::RequirementsTxt,
            ManifestType::GoMod,
            ManifestType::ComposerJson,
            ManifestType::Gemfile,
            ManifestType::PomXml,
            ManifestType::BuildGradle,
            ManifestType::CMakeLists,
            ManifestType::PubspecYaml,
        ];

        for manifest_type in manifest_types {
            let manifest_path = dir.join(manifest_type.filename());
            if manifest_path.exists() {
                if let Some(signal) = self.parse_manifest(&manifest_path, manifest_type) {
                    signals.push(signal);
                }
            }
        }

        // Special handling for .csproj files (glob pattern)
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "csproj") {
                    if let Some(signal) = self.parse_manifest(&path, ManifestType::Csproj) {
                        signals.push(signal);
                    }
                }
            }
        }

        // Supplement manifest deps with import-extracted packages.
        // Only scan if we found at least one manifest (confirms this is a project dir).
        if signals.len() > signals_start {
            let mut import_deps: HashSet<String> = HashSet::new();
            let mut files_scanned = 0u32;
            const MAX_SOURCE_FILES: u32 = 50;

            // Scan source files in the manifest directory.
            // `files_scanned` must increment on every file we READ, not only
            // when imports are found — the old "only on hit" counting let a
            // directory of import-less files blow past the cap and read (and
            // on OneDrive, hydrate) every one of them.
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if files_scanned >= MAX_SOURCE_FILES {
                        break;
                    }
                    let path = entry.path();
                    if path.is_file() {
                        files_scanned += 1;
                        let extracted = extract_imports_from_source(&path);
                        if !extracted.is_empty() {
                            import_deps.extend(extracted);
                        }
                    }
                }
            }

            // Also scan src/ subdirectory if it exists (common convention)
            let src_dir = dir.join("src");
            if src_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&src_dir) {
                    for entry in entries.flatten() {
                        if files_scanned >= MAX_SOURCE_FILES {
                            break;
                        }
                        let path = entry.path();
                        if path.is_file() {
                            files_scanned += 1;
                            let extracted = extract_imports_from_source(&path);
                            if !extracted.is_empty() {
                                import_deps.extend(extracted);
                            }
                        }
                    }
                }
            }

            // Merge unique import deps into ALL signals from this directory
            if !import_deps.is_empty() {
                for signal in &mut signals[signals_start..] {
                    for dep in &import_deps {
                        if !signal.dependencies.contains(dep)
                            && !signal.dev_dependencies.contains(dep)
                        {
                            signal.dependencies.push(dep.clone());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_manifest(&self, path: &Path, manifest_type: ManifestType) -> Option<ProjectSignal> {
        // A dehydrated manifest (cloud-only) isn't worth forcing a download.
        if is_cloud_placeholder(path) {
            return None;
        }
        let content = fs::read_to_string(path).ok()?;

        let mut signal = ProjectSignal {
            manifest_type,
            manifest_path: path.to_path_buf(),
            project_name: None,
            languages: vec![manifest_type.language().to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: chrono::Utc::now().to_rfc3339(),
            project_license: None,
            project_relevance: compute_project_relevance(path),
        };

        match manifest_type {
            ManifestType::CargoToml => self.parse_cargo_toml(&content, &mut signal),
            ManifestType::PackageJson => self.parse_package_json(&content, &mut signal),
            ManifestType::PyprojectToml => self.parse_pyproject_toml(&content, &mut signal),
            ManifestType::RequirementsTxt => self.parse_requirements_txt(&content, &mut signal),
            ManifestType::GoMod => self.parse_go_mod(&content, &mut signal),
            ManifestType::ComposerJson => self.parse_composer_json(&content, &mut signal),
            ManifestType::Gemfile => self.parse_gemfile(&content, &mut signal),
            ManifestType::PomXml => self.parse_pom_xml(&content, &mut signal),
            ManifestType::BuildGradle => self.parse_build_gradle(&content, &mut signal),
            ManifestType::Csproj => self.parse_csproj(&content, &mut signal),
            ManifestType::PubspecYaml => self.parse_pubspec_yaml(&content, &mut signal),
            ManifestType::CMakeLists => {} // language detected; no standard dep manifest
        }

        Some(signal)
    }

    pub(crate) fn parse_cargo_toml(&self, content: &str, signal: &mut ProjectSignal) {
        // Parse with toml crate if available, otherwise use regex
        // Extract package name
        if let Some(name) = extract_toml_value(content, "name") {
            signal.project_name = Some(name);
        }

        // Extract license (SPDX identifier from [package] section)
        if let Some(license) = extract_toml_value(content, "license") {
            signal.project_license = Some(license);
        }

        // Extract dependencies
        let mut in_deps = false;
        let mut in_dev_deps = false;
        // `Some(spec)` while inside a `[target.<spec>.dependencies]` section.
        let mut current_target: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == "[dependencies]" || trimmed == "[workspace.dependencies]" {
                in_deps = true;
                in_dev_deps = false;
                current_target = None;
                continue;
            } else if trimmed == "[dev-dependencies]" {
                in_deps = false;
                in_dev_deps = true;
                current_target = None;
                continue;
            } else if let Some(spec) = parse_target_deps_header(trimmed) {
                // Platform-gated runtime deps, e.g. [target.'cfg(windows)'.dependencies].
                // Previously skipped entirely (windows-sys, winreg, libc were invisible).
                in_deps = true;
                in_dev_deps = false;
                current_target = Some(spec);
                continue;
            } else if trimmed.starts_with('[') {
                in_deps = false;
                in_dev_deps = false;
                current_target = None;
                continue;
            }

            if (in_deps || in_dev_deps) && !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some((dep_name, dep_value)) = trimmed.split_once('=') {
                    let dep = dep_name.trim().to_string();
                    // Skip local path/git deps (e.g. `foo = { path = "..." }`).
                    // These are the user's own crates or vendored code with no
                    // crates.io presence — tracking them as external dependencies
                    // only produces false-positive "unmonitored" blind spots.
                    if is_local_cargo_dep(dep_value) {
                        continue;
                    }
                    if !dep.is_empty() {
                        if let Some(ref target) = current_target {
                            signal
                                .target_dependencies
                                .push((dep.clone(), target.clone()));
                            self.detect_rust_framework(&dep, signal);
                        } else if in_dev_deps {
                            signal.dev_dependencies.push(dep);
                        } else {
                            signal.dependencies.push(dep.clone());
                            // Detect frameworks
                            self.detect_rust_framework(&dep, signal);
                        }
                    }
                }
            }
        }
    }

    fn detect_rust_framework(&self, dep: &str, signal: &mut ProjectSignal) {
        // Only actual web/app frameworks — runtimes, ORMs, and serialization
        // libraries are detected via is_notable_dependency() in ace/context.rs
        let frameworks = [
            ("tauri", "tauri"),
            ("actix", "actix-web"),
            ("axum", "axum"),
            ("rocket", "rocket"),
            ("warp", "warp"),
        ];

        let dep_lower = dep.to_lowercase();
        for (pattern, framework) in frameworks {
            let matches = dep_lower == pattern || dep_lower.starts_with(&format!("{pattern}-"));
            if matches && !signal.frameworks.contains(&framework.to_string()) {
                signal.frameworks.push(framework.to_string());
            }
        }
    }

    pub(crate) fn parse_package_json(&self, content: &str, signal: &mut ProjectSignal) {
        let Ok(pkg) = serde_json::from_str::<serde_json::Value>(content) else {
            return;
        };

        if let Some(name) = pkg.get("name").and_then(|v| v.as_str()) {
            signal.project_name = Some(name.to_string());
        }

        // Extract license field (SPDX identifier)
        if let Some(license) = pkg.get("license").and_then(|v| v.as_str()) {
            signal.project_license = Some(license.to_string());
        }

        if let Some(deps) = pkg.get("dependencies").and_then(|v| v.as_object()) {
            for (key, val) in deps {
                // Skip local/non-registry specs (file:/link:/workspace:/git/url)
                // — sibling or vendored packages produce false-positive blind spots.
                if is_local_npm_spec(val.as_str().unwrap_or("")) {
                    continue;
                }
                signal.dependencies.push(key.clone());
                self.detect_js_framework(key, signal);
            }
        }

        if let Some(dev_deps) = pkg.get("devDependencies").and_then(|v| v.as_object()) {
            for (key, val) in dev_deps {
                if is_local_npm_spec(val.as_str().unwrap_or("")) {
                    continue;
                }
                signal.dev_dependencies.push(key.clone());
            }
            // Detect TypeScript from devDependencies
            if dev_deps.contains_key("typescript")
                && !signal.languages.contains(&"typescript".to_string())
            {
                signal.languages.push("typescript".to_string());
            }
        }

        // Also check dependencies for TypeScript / @types
        if let Some(deps) = pkg.get("dependencies").and_then(|v| v.as_object()) {
            if (deps.contains_key("typescript") || deps.keys().any(|k| k.starts_with("@types/")))
                && !signal.languages.contains(&"typescript".to_string())
            {
                signal.languages.push("typescript".to_string());
            }
        }
    }

    fn detect_js_framework(&self, dep: &str, signal: &mut ProjectSignal) {
        // Only actual web/app frameworks — ORMs, build tools, and CSS utility
        // libraries are detected via is_notable_dependency() in ace/context.rs
        let frameworks = [
            ("react", "react"),
            ("vue", "vue"),
            ("angular", "angular"),
            ("svelte", "svelte"),
            ("next", "next.js"),
            ("nuxt", "nuxt"),
            ("express", "express"),
            ("fastify", "fastify"),
            ("nestjs", "nestjs"),
            ("electron", "electron"),
        ];

        let dep_lower = dep.to_lowercase();
        for (pattern, framework) in frameworks {
            let matches = dep_lower == pattern
                || dep_lower.starts_with(&format!("{pattern}-"))
                || dep_lower.ends_with(&format!("/{pattern}"));
            if matches && !signal.frameworks.contains(&framework.to_string()) {
                signal.frameworks.push(framework.to_string());
            }
        }
    }

    pub(crate) fn parse_pyproject_toml(&self, content: &str, signal: &mut ProjectSignal) {
        // Extract project name
        if let Some(name) = extract_toml_value(content, "name") {
            signal.project_name = Some(name);
        }

        // Look for common frameworks
        // Only actual web frameworks — ML libraries, ORMs, and task queues
        // are detected via is_notable_dependency() in ace/context.rs
        let python_frameworks = [
            ("django", "django"),
            ("flask", "flask"),
            ("fastapi", "fastapi"),
        ];

        let content_lower = content.to_lowercase();
        for (pattern, framework) in python_frameworks {
            if content_lower.contains(pattern)
                && !signal.frameworks.contains(&framework.to_string())
            {
                signal.frameworks.push(framework.to_string());
                signal.dependencies.push(pattern.to_string());
            }
        }
    }

    fn parse_requirements_txt(&self, content: &str, signal: &mut ProjectSignal) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }

            // Extract package name (before version specifier)
            let dep_name = trimmed
                .split(&['=', '>', '<', '~', '!', '['][..])
                .next()
                .unwrap_or(trimmed)
                .trim()
                .to_string();

            if !dep_name.is_empty() {
                signal.dependencies.push(dep_name.clone());

                // Detect frameworks
                // Only actual web frameworks — ML/data libraries are
                // detected via is_notable_dependency() in ace/context.rs
                let frameworks = [
                    ("django", "django"),
                    ("flask", "flask"),
                    ("fastapi", "fastapi"),
                ];

                for (pattern, framework) in frameworks {
                    if dep_name.to_lowercase().contains(pattern)
                        && !signal.frameworks.contains(&framework.to_string())
                    {
                        signal.frameworks.push(framework.to_string());
                    }
                }
            }
        }
    }

    pub(crate) fn parse_go_mod(&self, content: &str, signal: &mut ProjectSignal) {
        // Extract module name
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix("module ") {
                signal.project_name = Some(name.trim().to_string());
                break;
            }
        }

        // Extract require dependencies
        let mut in_require = false;
        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("require (") {
                in_require = true;
                continue;
            } else if trimmed == ")" {
                in_require = false;
                continue;
            }

            if in_require && !trimmed.is_empty() {
                if let Some(dep_path) = trimmed.split_whitespace().next() {
                    signal.dependencies.push(dep_path.to_string());

                    // Detect Go frameworks
                    // Only actual web/CLI frameworks — ORMs are detected
                    // via is_notable_dependency() in ace/context.rs
                    let frameworks = [
                        ("gin-gonic", "gin"),
                        ("echo", "echo"),
                        ("fiber", "fiber"),
                        ("cobra", "cobra"),
                    ];

                    for (pattern, framework) in frameworks {
                        if dep_path.contains(pattern)
                            && !signal.frameworks.contains(&framework.to_string())
                        {
                            signal.frameworks.push(framework.to_string());
                        }
                    }
                }
            }
        }
    }

    // ========================================================================
    // Manifest parsers for the remaining ecosystems (PHP / Ruby / Java / C# / Dart).
    // Each extracts dependency NAMES (the version-bearing path stays the lockfile
    // parsers below + ACE's version resolver), keyed to the ecosystem OSV expects:
    // composer "vendor/pkg", maven "groupId:artifactId", NuGet/RubyGems/Pub package ids.
    // Mirrors parse_package_json / parse_go_mod; framework detection via push_frameworks.
    // ========================================================================

    /// composer.json (PHP/Packagist). `require` keys are "vendor/package"; ext-*/lib-*/php meta skipped.
    pub(crate) fn parse_composer_json(&self, content: &str, signal: &mut ProjectSignal) {
        let Ok(pkg) = serde_json::from_str::<serde_json::Value>(content) else {
            return;
        };
        if let Some(name) = pkg.get("name").and_then(|v| v.as_str()) {
            signal.project_name = Some(name.to_string());
        }
        if let Some(license) = pkg.get("license").and_then(|v| v.as_str()) {
            signal.project_license = Some(license.to_string());
        }
        let is_pkg = |k: &str| k.contains('/') && !k.starts_with("ext-") && !k.starts_with("lib-");
        if let Some(deps) = pkg.get("require").and_then(|v| v.as_object()) {
            for key in deps.keys() {
                if is_pkg(key) {
                    signal.dependencies.push(key.clone());
                    Self::push_frameworks(key, PHP_FRAMEWORKS, signal);
                }
            }
        }
        if let Some(dev) = pkg.get("require-dev").and_then(|v| v.as_object()) {
            for key in dev.keys() {
                if is_pkg(key) {
                    signal.dev_dependencies.push(key.clone());
                }
            }
        }
    }

    /// Gemfile (Ruby/RubyGems). Lines like `gem "name", "~> 1.2"`; `:development`/`:test` groups -> dev.
    pub(crate) fn parse_gemfile(&self, content: &str, signal: &mut ProjectSignal) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some(rest) = trimmed.strip_prefix("gem ") else {
                continue;
            };
            let Some(name) = Self::extract_first_quoted(rest) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let is_dev = rest.contains(":development") || rest.contains(":test");
            if is_dev {
                signal.dev_dependencies.push(name);
            } else {
                signal.dependencies.push(name.clone());
                Self::push_frameworks(&name, RUBY_FRAMEWORKS, signal);
            }
        }
    }

    /// pom.xml (Java/Maven). Each `<dependency>` -> "groupId:artifactId"; `<scope>test</scope>` -> dev.
    pub(crate) fn parse_pom_xml(&self, content: &str, signal: &mut ProjectSignal) {
        if let Some(name) = Self::extract_xml_tag(content, "name") {
            signal.project_name = Some(name);
        }
        for block in content.split("<dependency>").skip(1) {
            let block = &block[..block.find("</dependency>").unwrap_or(block.len())];
            let (Some(group), Some(artifact)) = (
                Self::extract_xml_tag(block, "groupId"),
                Self::extract_xml_tag(block, "artifactId"),
            ) else {
                continue;
            };
            let coord = format!("{group}:{artifact}");
            if Self::extract_xml_tag(block, "scope").as_deref() == Some("test") {
                signal.dev_dependencies.push(coord);
            } else {
                signal.dependencies.push(coord.clone());
                Self::push_frameworks(&coord, JVM_FRAMEWORKS, signal);
            }
        }
    }

    /// build.gradle / build.gradle.kts (Java-Kotlin/Maven coords). `implementation 'g:a:v'`,
    /// `api("g:a:v")`, etc. `test*` configurations -> dev.
    pub(crate) fn parse_build_gradle(&self, content: &str, signal: &mut ProjectSignal) {
        const CONFIGS: &[&str] = &[
            "implementation",
            "api",
            "compileOnly",
            "runtimeOnly",
            "compile",
            "testImplementation",
            "testRuntimeOnly",
            "testCompileOnly",
            "kapt",
            "annotationProcessor",
        ];
        for line in content.lines() {
            let trimmed = line.trim();
            let Some(config) = CONFIGS.iter().find(|c| trimmed.starts_with(**c)) else {
                continue;
            };
            let Some(coord) = Self::extract_first_quoted(trimmed) else {
                continue;
            };
            // Maven coordinate is group:artifact:version — key on group:artifact.
            let parts: Vec<&str> = coord.split(':').collect();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
                continue;
            }
            let ga = format!("{}:{}", parts[0], parts[1]);
            if config.starts_with("test") {
                signal.dev_dependencies.push(ga);
            } else {
                signal.dependencies.push(ga.clone());
                Self::push_frameworks(&ga, JVM_FRAMEWORKS, signal);
            }
        }
    }

    /// *.csproj (C#/.NET/NuGet). `<PackageReference Include="Name" Version="..." />` -> "Name".
    pub(crate) fn parse_csproj(&self, content: &str, signal: &mut ProjectSignal) {
        for block in content.split("<PackageReference").skip(1) {
            // Bound the search to this element (self-closing `/>` or the opening tag's `>`).
            let end = block
                .find("/>")
                .or_else(|| block.find('>'))
                .unwrap_or(block.len());
            let elem = &block[..end];
            if let Some(name) = Self::extract_xml_attr(elem, "Include") {
                if !name.is_empty() {
                    signal.dependencies.push(name.clone());
                    Self::push_frameworks(&name, DOTNET_FRAMEWORKS, signal);
                }
            }
        }
    }

    /// pubspec.yaml (Dart/Pub). Top-level `dependencies:` / `dev_dependencies:` blocks; the
    /// 2-space-indented keys are package names (nested `sdk:`/`git:` lines are ignored).
    pub(crate) fn parse_pubspec_yaml(&self, content: &str, signal: &mut ProjectSignal) {
        if let Some(name) = Self::extract_yaml_top_value(content, "name") {
            signal.project_name = Some(name);
        }
        // Some(true) = dependencies, Some(false) = dev_dependencies, None = elsewhere.
        let mut section: Option<bool> = None;
        for line in content.lines() {
            if line.starts_with("dependencies:") {
                section = Some(true);
                continue;
            }
            if line.starts_with("dev_dependencies:") {
                section = Some(false);
                continue;
            }
            // A non-indented, non-empty line ends the current block.
            if !line.starts_with(char::is_whitespace) && !line.trim().is_empty() {
                section = None;
                continue;
            }
            let Some(is_runtime) = section else {
                continue;
            };
            let indent = line.len() - line.trim_start().len();
            let t = line.trim();
            // Only first-level (2-space) keys are package names; deeper lines are sub-fields.
            if indent != 2 || t.is_empty() || t.starts_with('#') {
                continue;
            }
            let Some(name) = t.split(':').next().map(|s| s.trim().to_string()) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if is_runtime {
                signal.dependencies.push(name.clone());
                Self::push_frameworks(&name, DART_FRAMEWORKS, signal);
            } else {
                signal.dev_dependencies.push(name);
            }
        }
    }

    /// Push any framework whose pattern is a substring of `dep` (case-insensitive), deduped.
    fn push_frameworks(dep: &str, table: &[(&str, &str)], signal: &mut ProjectSignal) {
        let d = dep.to_lowercase();
        for (pat, fw) in table {
            if d.contains(pat) && !signal.frameworks.iter().any(|f| f == fw) {
                signal.frameworks.push((*fw).to_string());
            }
        }
    }

    /// First single- or double-quoted substring (the quote char must match to close).
    fn extract_first_quoted(s: &str) -> Option<String> {
        let bytes = s.as_bytes();
        let start = bytes.iter().position(|&b| b == b'"' || b == b'\'')?;
        let quote = bytes[start] as char;
        let rest = &s[start + 1..];
        let end = rest.find(quote)?;
        Some(rest[..end].to_string())
    }

    /// Inner text of the first `<tag>...</tag>` (trimmed). Namespaced/attributed tags not handled.
    fn extract_xml_tag(s: &str, tag: &str) -> Option<String> {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let start = s.find(&open)? + open.len();
        let end = s[start..].find(&close)? + start;
        Some(s[start..end].trim().to_string())
    }

    /// Value of the first `attr="..."` attribute.
    fn extract_xml_attr(s: &str, attr: &str) -> Option<String> {
        let needle = format!("{attr}=\"");
        let start = s.find(&needle)? + needle.len();
        let end = s[start..].find('"')? + start;
        Some(s[start..end].to_string())
    }

    /// Value of a top-level (non-indented) `key:` line in simple YAML.
    fn extract_yaml_top_value(content: &str, key: &str) -> Option<String> {
        let prefix = format!("{key}:");
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix(&prefix) {
                let v = rest.trim().trim_matches(|c| c == '"' || c == '\'').trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    // ========================================================================
    // Lockfile Parsers — discover transitive (indirect) dependencies
    // ========================================================================

    /// Parse a Cargo.lock file and return (package_name, version) pairs.
    /// The root project package (matching the project_name from Cargo.toml) is excluded.
    pub(crate) fn parse_cargo_lock(content: &str) -> Vec<(String, String)> {
        let mut packages = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_version: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[[package]]" {
                // Flush previous package
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    packages.push((name, version));
                }
            } else if let Some(rest) = trimmed.strip_prefix("name = ") {
                current_name = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("version = ") {
                current_version = Some(rest.trim_matches('"').to_string());
            }
        }
        // Don't forget the last package
        if let (Some(name), Some(version)) = (current_name, current_version) {
            packages.push((name, version));
        }

        packages
    }

    /// Parse a package-lock.json (v1/v2/v3) and return (package_name, version) pairs.
    /// Skips nested node_modules (transitive-of-transitive) and the root "" entry.
    pub(crate) fn parse_package_lock_json(content: &str) -> Vec<(String, String)> {
        let Ok(lock) = serde_json::from_str::<serde_json::Value>(content) else {
            return Vec::new();
        };

        let mut packages = Vec::new();

        // v2/v3 format uses "packages" key
        if let Some(pkgs) = lock.get("packages").and_then(|v| v.as_object()) {
            for (key, value) in pkgs {
                // Skip the root "" entry
                if key.is_empty() {
                    continue;
                }
                // Extract package name from path (e.g., "node_modules/@scope/pkg" -> "@scope/pkg")
                let name = key.strip_prefix("node_modules/").unwrap_or(key);
                // Skip nested node_modules (too deep — we want first-level transitive only)
                if name.contains("node_modules/") {
                    continue;
                }
                if let Some(version) = value.get("version").and_then(|v| v.as_str()) {
                    packages.push((name.to_string(), version.to_string()));
                }
            }
        }
        // v1 format uses "dependencies" key (older lockfile format)
        else if let Some(deps) = lock.get("dependencies").and_then(|v| v.as_object()) {
            for (name, value) in deps {
                if let Some(version) = value.get("version").and_then(|v| v.as_str()) {
                    packages.push((name.clone(), version.to_string()));
                }
            }
        }

        packages
    }

    /// Parse a pnpm-lock.yaml (v5/v6/v9) and return (package_name, version) pairs.
    /// Uses focused line-by-line parsing (no YAML crate needed) since the `packages:`
    /// section has a predictable structure: top-level keys at 2-space indent.
    ///
    /// v5/v6 keys: `  /@scope/pkg/1.2.3:` or `  /pkg/1.2.3:`
    /// v9 keys:    `  @scope/pkg@1.2.3:` or `  pkg@1.2.3:`
    /// Some entries have a nested `version:` field instead of version-in-key.
    pub(crate) fn parse_pnpm_lock_yaml(content: &str) -> Vec<(String, String)> {
        let mut packages = Vec::new();
        let mut in_packages = false;
        let mut pending_name: Option<String> = None;

        for line in content.lines() {
            if line.starts_with("packages:") {
                in_packages = true;
                continue;
            }
            if !in_packages {
                continue;
            }
            // A new top-level key (no indent) ends the packages section
            if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }

            let trimmed = line.trim();

            // Top-level package key: exactly 2 spaces (or 1 tab) of indentation + ends with ':'
            let is_package_key = (line.starts_with("  ") && !line.starts_with("    "))
                || (line.starts_with('\t') && !line.starts_with("\t\t"));

            if is_package_key && trimmed.ends_with(':') {
                pending_name = None;
                let key = trimmed.trim_end_matches(':');
                // Strip optional YAML quoting
                let key = key.trim_matches('\'').trim_matches('"');
                if let Some((name, ver)) = parse_pnpm_package_key(key) {
                    packages.push((name, ver));
                } else {
                    // Version might be in a nested `version:` field
                    let name = key.trim_start_matches('/');
                    if !name.is_empty() {
                        pending_name = Some(name.to_string());
                    }
                }
            } else if let Some(ref name) = pending_name {
                if let Some(rest) = trimmed.strip_prefix("version:") {
                    let ver = rest.trim().trim_matches('\'').trim_matches('"');
                    // Strip pnpm peer-dep suffixes like `1.2.3(react@18.2.0)`
                    let ver = ver.split('(').next().unwrap_or(ver).trim();
                    if !ver.is_empty() {
                        packages.push((name.clone(), ver.to_string()));
                    }
                    pending_name = None;
                }
            }
        }

        packages
    }

    /// Parse a yarn.lock (v1 classic) and return (package_name, version) pairs.
    /// Keys look like `"lodash@^4.17.20":` and resolved version appears as `version "4.17.21"`.
    pub(crate) fn parse_yarn_lock(content: &str) -> Vec<(String, String)> {
        let mut packages = Vec::new();
        let mut current_name: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            if !trimmed.starts_with('#')
                && !trimmed.is_empty()
                && !line.starts_with(' ')
                && !line.starts_with('\t')
            {
                // Top-level key line like `"lodash@^4.17.20":` or `lodash@^4.17.20:`
                let clean = trimmed.trim_end_matches(':').replace('"', "");
                // Take the first specifier (before any comma) and extract the package name
                if let Some(spec) = clean.split(',').next() {
                    let spec = spec.trim();
                    // Split at last '@' that isn't at position 0 (scoped packages start with @)
                    if let Some(at_pos) = spec.rfind('@').filter(|&p| p > 0) {
                        current_name = Some(spec[..at_pos].to_string());
                    }
                }
            } else if let Some(ref name) = current_name {
                if let Some(rest) = trimmed.strip_prefix("version ") {
                    let version = rest.trim_matches('"').to_string();
                    if !version.is_empty() {
                        packages.push((name.clone(), version));
                    }
                    current_name = None;
                }
            }
        }

        packages
    }

    /// Parse a requirements.txt and return (package_name, version) for EXACT (`==`) pins only.
    /// A `name==X` pin IS the installed version (a pinned requirements.txt is the lock for the
    /// stack), so it plays the same role poetry.lock does for Poetry projects. Non-exact
    /// specifiers (`>=`, `~=`, ranges, `===` arbitrary equality) yield no single resolved
    /// version and are skipped. Environment markers (`; python_version<...`), extras
    /// (`pkg[extra]==`), and inline comments are stripped.
    pub(crate) fn parse_requirements_txt_pins(content: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }
            // Drop environment markers (after `;`) and inline comments (after ` #`).
            let core = trimmed
                .split(';')
                .next()
                .unwrap_or(trimmed)
                .split(" #")
                .next()
                .unwrap_or(trimmed)
                .trim();
            let name = core
                .split(&['=', '>', '<', '~', '!', '['][..])
                .next()
                .unwrap_or(core)
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let Some(idx) = core.find("==") else {
                continue; // only exact pins carry a resolved version
            };
            let after = &core[idx + 2..];
            if after.starts_with('=') {
                continue; // `===` arbitrary equality — not a clean version
            }
            let version: String = after
                .trim()
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != ',')
                .collect();
            let version = version.trim().to_string();
            if !version.is_empty() {
                out.push((name, version));
            }
        }
        out
    }

    /// Parse a poetry.lock file and return (package_name, version) pairs.
    /// Format: TOML with `[[package]]` sections containing `name` and `version` fields.
    pub(crate) fn parse_poetry_lock(content: &str) -> Vec<(String, String)> {
        let mut packages = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_version: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[[package]]" {
                if let (Some(name), Some(version)) = (current_name.take(), current_version.take()) {
                    packages.push((name, version));
                }
            } else if let Some(rest) = trimmed.strip_prefix("name = ") {
                current_name = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("version = ") {
                current_version = Some(rest.trim_matches('"').to_string());
            }
        }
        if let (Some(name), Some(version)) = (current_name, current_version) {
            packages.push((name, version));
        }

        packages
    }

    /// Parse a go.sum file and return (module_name, version) pairs.
    /// Format: `module version hash` per line. Each module appears twice
    /// (once for module, once for go.mod). Deduplicates by (module, version).
    pub(crate) fn parse_go_sum(content: &str) -> Vec<(String, String)> {
        let mut seen = std::collections::HashSet::new();
        let mut packages = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
            if parts.len() < 2 {
                continue;
            }
            let module = parts[0];
            let version_raw = parts[1];
            // Strip /go.mod suffix from version
            let version = version_raw.strip_suffix("/go.mod").unwrap_or(version_raw);
            // Strip the "v" prefix for consistency with semver
            let version_clean = version.trim_start_matches('v');
            if version_clean.is_empty() {
                continue;
            }
            let key = (module.to_string(), version_clean.to_string());
            if seen.insert(key.clone()) {
                packages.push(key);
            }
        }

        packages
    }

    /// Parse a Gemfile.lock file and return (gem_name, version) pairs.
    /// Format: Indentation-based. Gems listed under `GEM > specs:` section
    /// as `    gem_name (version)` (4-space indent = direct, 6-space = transitive).
    pub(crate) fn parse_gemfile_lock(content: &str) -> Vec<(String, String)> {
        let mut packages = Vec::new();
        let mut in_gem_specs = false;

        for line in content.lines() {
            // Detect "GEM" section
            if line == "GEM" {
                in_gem_specs = false;
                continue;
            }
            // Detect "  specs:" within GEM section
            if line == "  specs:" {
                in_gem_specs = true;
                continue;
            }
            // A non-indented line (other than GEM) ends the specs section
            if !line.starts_with(' ') && !line.is_empty() {
                if in_gem_specs {
                    in_gem_specs = false;
                }
                continue;
            }

            if !in_gem_specs {
                continue;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Gem entries are at exactly 4-space indent; sub-dependency constraints
            // are at 6+ spaces. Only parse 4-space-indented lines as actual gems.
            let indent = line.len() - line.trim_start().len();
            if indent != 4 {
                continue;
            }

            // Gem lines: "    gem_name (1.2.3)" — exactly 4-space indent with parenthesized version
            if let Some(paren_start) = trimmed.rfind('(') {
                if let Some(paren_end) = trimmed.rfind(')') {
                    if paren_end > paren_start {
                        let name = trimmed[..paren_start].trim();
                        let version = &trimmed[paren_start + 1..paren_end];
                        if !name.is_empty() && !version.is_empty() && !name.contains(' ') {
                            packages.push((name.to_string(), version.to_string()));
                        }
                    }
                }
            }
        }

        packages
    }

    pub(crate) fn parse_composer_lock(content: &str) -> Vec<(String, String)> {
        let parsed: serde_json::Value = match serde_json::from_str(content) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let mut packages = Vec::new();

        for key in &["packages", "packages-dev"] {
            if let Some(arr) = parsed.get(key).and_then(|v| v.as_array()) {
                for entry in arr {
                    let name = entry.get("name").and_then(|v| v.as_str());
                    let version = entry.get("version").and_then(|v| v.as_str());
                    if let (Some(n), Some(v)) = (name, version) {
                        let v = v.strip_prefix('v').unwrap_or(v);
                        if !n.is_empty() && !v.is_empty() {
                            packages.push((n.to_string(), v.to_string()));
                        }
                    }
                }
            }
        }

        packages
    }

    // ========================================================================
    // Edge parsers — capture parent->child graph (alongside flatten parsers)
    // ========================================================================

    /// Parse a Cargo.lock and return parent->child dependency edges.
    /// Each `[[package]]` block carries `name`, `version`, and an optional
    /// `dependencies = [ "dep", "dep 1.2.3", ... ]` array. We emit one edge per
    /// child. Cargo.lock does not separate dev deps in its resolved graph, so all
    /// edges are `Runtime`. Robust to malformed input — returns what it can.
    pub(crate) fn parse_cargo_lock_edges(content: &str) -> Vec<DependencyEdge> {
        let mut edges = Vec::new();
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut in_deps = false;

        // Flush the current package's dependency list as edges.
        fn flush(
            edges: &mut Vec<DependencyEdge>,
            name: &Option<String>,
            version: &Option<String>,
            children: &[(String, Option<String>)],
        ) {
            if let Some(parent) = name {
                for (child, child_version) in children {
                    edges.push(DependencyEdge {
                        parent: parent.clone(),
                        parent_version: version.clone(),
                        child: child.clone(),
                        child_version: child_version.clone(),
                        scope: EdgeScope::Runtime,
                    });
                }
            }
        }

        let mut children: Vec<(String, Option<String>)> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == "[[package]]" {
                flush(&mut edges, &name, &version, &children);
                name = None;
                version = None;
                children.clear();
                in_deps = false;
                continue;
            }

            if in_deps {
                // Inside a `dependencies = [` array until the closing `]`.
                if trimmed.contains(']') {
                    in_deps = false;
                    // A single-line array may close on the same logical line; the
                    // entries themselves are handled below for the multi-line form.
                    let inner = trimmed.trim_end_matches(']');
                    for part in inner.split(',') {
                        let part = part.trim();
                        if part.is_empty() || part == "[" {
                            continue;
                        }
                        let (c, v) = split_cargo_dep_spec(part);
                        if !c.is_empty() {
                            children.push((c, v));
                        }
                    }
                    continue;
                }
                let part = trimmed.trim_end_matches(',');
                if !part.is_empty() {
                    let (c, v) = split_cargo_dep_spec(part);
                    if !c.is_empty() {
                        children.push((c, v));
                    }
                }
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("name = ") {
                name = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("version = ") {
                version = Some(rest.trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("dependencies = ") {
                let rest = rest.trim();
                // Inline single-line array: dependencies = ["a", "b 1.0"]
                if let Some(inner) = rest.strip_prefix('[') {
                    if let Some(inner) = inner.strip_suffix(']') {
                        for part in inner.split(',') {
                            let part = part.trim();
                            if part.is_empty() {
                                continue;
                            }
                            let (c, v) = split_cargo_dep_spec(part);
                            if !c.is_empty() {
                                children.push((c, v));
                            }
                        }
                    } else {
                        // Multi-line array begins here.
                        in_deps = true;
                    }
                }
            }
        }

        flush(&mut edges, &name, &version, &children);
        edges
    }

    /// Parse a package-lock.json (v2/v3 `packages` map, v1 `dependencies` tree)
    /// and return parent->child edges. Root edges use the [`ROOT_PARENT`] sentinel.
    /// `dependencies` keys are `Runtime`, `devDependencies` are `Dev`.
    /// Robust to malformed input — returns an empty Vec on parse failure.
    pub(crate) fn parse_package_lock_edges(content: &str) -> Vec<DependencyEdge> {
        let Ok(lock) = serde_json::from_str::<serde_json::Value>(content) else {
            return Vec::new();
        };

        let mut edges = Vec::new();

        // v2/v3: a `packages` map keyed by node_modules path. "" is the root.
        if let Some(pkgs) = lock.get("packages").and_then(|v| v.as_object()) {
            for (key, value) in pkgs {
                // Skip deeply nested node_modules — first-level edges only.
                let name_part = key.strip_prefix("node_modules/").unwrap_or(key);
                if name_part.contains("node_modules/") {
                    continue;
                }
                let parent = if key.is_empty() {
                    ROOT_PARENT.to_string()
                } else {
                    name_part.to_string()
                };
                let parent_version = value
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                for (section, scope) in [
                    ("dependencies", EdgeScope::Runtime),
                    ("devDependencies", EdgeScope::Dev),
                    ("optionalDependencies", EdgeScope::Dev),
                    ("peerDependencies", EdgeScope::Runtime),
                ] {
                    if let Some(deps) = value.get(section).and_then(|v| v.as_object()) {
                        for (child, spec) in deps {
                            edges.push(DependencyEdge {
                                parent: parent.clone(),
                                parent_version: parent_version.clone(),
                                child: child.clone(),
                                child_version: spec.as_str().map(str::to_string),
                                scope,
                            });
                        }
                    }
                }
            }
            return edges;
        }

        // v1 fallback: a `dependencies` tree where each node may carry `requires`
        // (its runtime children) and a `dev` flag. Emit root->dep edges and
        // dep->requires edges.
        if let Some(deps) = lock.get("dependencies").and_then(|v| v.as_object()) {
            for (name, value) in deps {
                let is_dev = value.get("dev").and_then(|v| v.as_bool()).unwrap_or(false);
                let version = value
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                edges.push(DependencyEdge {
                    parent: ROOT_PARENT.to_string(),
                    parent_version: None,
                    child: name.clone(),
                    child_version: version.clone(),
                    scope: if is_dev {
                        EdgeScope::Dev
                    } else {
                        EdgeScope::Runtime
                    },
                });
                if let Some(requires) = value.get("requires").and_then(|v| v.as_object()) {
                    for (child, spec) in requires {
                        edges.push(DependencyEdge {
                            parent: name.clone(),
                            parent_version: version.clone(),
                            child: child.clone(),
                            child_version: spec.as_str().map(str::to_string),
                            scope: if is_dev {
                                EdgeScope::Dev
                            } else {
                                EdgeScope::Runtime
                            },
                        });
                    }
                }
            }
        }

        edges
    }

    /// Parse a pnpm-lock.yaml and return parent->child edges. Each top-level
    /// package entry's nested `dependencies:` sub-map yields `Runtime` children;
    /// `devDependencies:`/`optionalDependencies:` yield `Dev` children. Reuses the
    /// same 2-space top-level indent convention as [`Self::parse_pnpm_lock_yaml`].
    /// Robust to malformed input.
    pub(crate) fn parse_pnpm_lock_edges(content: &str) -> Vec<DependencyEdge> {
        let mut edges = Vec::new();
        let mut in_packages = false;
        let mut current_parent: Option<(String, Option<String>)> = None;
        let mut current_scope: Option<EdgeScope> = None;

        for line in content.lines() {
            if line.starts_with("packages:") {
                in_packages = true;
                continue;
            }
            if !in_packages {
                continue;
            }
            // A new top-level (zero-indent) key ends the packages section.
            if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Top-level package key: exactly 2 spaces (or 1 tab) + ends with ':'.
            let is_package_key = (line.starts_with("  ") && !line.starts_with("   "))
                || (line.starts_with('\t') && !line.starts_with("\t\t"));

            if is_package_key && trimmed.ends_with(':') {
                let key = trimmed.trim_end_matches(':');
                let key = key.trim_matches('\'').trim_matches('"');
                let (name, version) = match parse_pnpm_package_key(key) {
                    Some((n, v)) => (n, Some(v)),
                    None => {
                        // v5/v6 single-segment form like `/express@4.18.2` (no
                        // nested `/version`) isn't handled by the key parser; split
                        // on the last `@` after stripping the leading slash.
                        let bare = key.trim_start_matches('/');
                        match bare.rfind('@').filter(|&p| p > 0) {
                            Some(at) => (bare[..at].to_string(), Some(bare[at + 1..].to_string())),
                            None => (bare.to_string(), None),
                        }
                    }
                };
                current_parent = if name.is_empty() {
                    None
                } else {
                    Some((name, version))
                };
                current_scope = None;
                continue;
            }

            // A sub-section header introduces a child map.
            let scope = match trimmed.trim_end_matches(':') {
                "dependencies" => Some(EdgeScope::Runtime),
                "devDependencies" | "optionalDependencies" => Some(EdgeScope::Dev),
                _ => None,
            };
            if trimmed.ends_with(':') && scope.is_some() {
                current_scope = scope;
                continue;
            }
            // Any other section header (resolution, engines, peerDependencies, ...)
            // ends the current child map.
            if trimmed.ends_with(':') && !trimmed.contains(' ') {
                current_scope = None;
                continue;
            }

            // A `child: specifier` line within an active dependency sub-map.
            if let (Some((parent, parent_version)), Some(scope)) =
                (current_parent.as_ref(), current_scope)
            {
                if let Some((child, spec)) = trimmed.split_once(':') {
                    let child = child.trim().trim_matches('\'').trim_matches('"');
                    if child.is_empty() {
                        continue;
                    }
                    let spec = spec.trim();
                    // pnpm v9 inline form: `specifier: ^1.0.0` then `version: 1.0.0`
                    // we keep the raw specifier as a best-effort child_version hint.
                    let child_version = if spec.is_empty() {
                        None
                    } else {
                        Some(spec.trim_matches('\'').trim_matches('"').to_string())
                    };
                    edges.push(DependencyEdge {
                        parent: parent.clone(),
                        parent_version: parent_version.clone(),
                        child: child.to_string(),
                        child_version,
                        scope,
                    });
                }
            }
        }

        edges
    }
}

/// Parse pnpm package key formats:
/// - v9: `@scope/pkg@1.2.3` or `pkg@1.2.3`
/// - v5/v6: `/@scope/pkg/1.2.3` or `/pkg/1.2.3`
fn parse_pnpm_package_key(key: &str) -> Option<(String, String)> {
    // v5/v6: starts with `/`, segments separated by `/`
    if let Some(rest) = key.strip_prefix('/') {
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        return match parts.len() {
            // /pkg/1.2.3
            2 => Some((parts[0].to_string(), parts[1].to_string())),
            // /@scope/pkg/1.2.3
            3 if parts[0].starts_with('@') => {
                let name = format!("{}/{}", parts[0], parts[1]);
                Some((name, parts[2].to_string()))
            }
            _ => None,
        };
    }

    // v9: `pkg@1.2.3` or `@scope/pkg@1.2.3`
    // Find the last `@` that isn't the scope prefix
    let at_pos = key.rfind('@').filter(|&p| p > 0)?;
    let name = &key[..at_pos];
    let version = &key[at_pos + 1..];
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name.to_string(), version.to_string()))
}

// ============================================================================
// Dependency Edge Parsers — capture the parent->child graph (Step 1: reachability)
// ============================================================================

/// The dependency scope of an edge. Cargo.lock does not separate dev in its
/// resolved graph (everything is `Runtime`); npm/pnpm distinguish dev/runtime,
/// and build-only scopes map to `Build`. `Unknown` is the safe default when a
/// lockfile gives us no scope signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeScope {
    Runtime,
    Dev,
    // `Build` and `Unknown` complete the scope model and back the table's
    // `scope` DEFAULT 'unknown'; they are not yet produced by the current
    // parsers (Cargo/npm/pnpm yield Runtime/Dev). Constructed once additional
    // ecosystems (build-only graphs) are wired in Increment 2.
    // REMOVE BY 2026-07-31: build-only graph scopes wired in increment 2
    #[allow(dead_code)]
    Build,
    // REMOVE BY 2026-07-31: unknown-scope fallback exercised once more ecosystems land (increment 2)
    #[allow(dead_code)]
    Unknown,
}

impl EdgeScope {
    /// Canonical lowercase string stored in the `scope` column.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            EdgeScope::Runtime => "runtime",
            EdgeScope::Dev => "dev",
            EdgeScope::Build => "build",
            EdgeScope::Unknown => "unknown",
        }
    }
}

/// A single parent->child dependency edge extracted from a lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyEdge {
    pub parent: String,
    pub parent_version: Option<String>,
    pub child: String,
    pub child_version: Option<String>,
    pub scope: EdgeScope,
}

/// Sentinel parent name for edges that originate from a lockfile's root project
/// node (e.g. the `""` entry in package-lock v2/v3). Reachability treats this as
/// a synthetic root so direct deps become BFS entry points.
pub(crate) const ROOT_PARENT: &str = "__root__";

/// Split a Cargo.lock `dependencies` array entry into (name, optional version).
/// Entries look like `"serde"` or `"serde 1.0.0"` or `"serde 1.0.0 (registry+...)"`.
fn split_cargo_dep_spec(spec: &str) -> (String, Option<String>) {
    let spec = spec.trim().trim_matches('"').trim();
    match spec.split_once(' ') {
        Some((name, rest)) => {
            // Version is the first whitespace-separated token after the name.
            let version = rest.split_whitespace().next().map(str::to_string);
            (name.to_string(), version)
        }
        None => (spec.to_string(), None),
    }
}

impl Default for ProjectScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Project Relevance Scoring
// ============================================================================

/// Compute relevance score for a project based on path patterns and git recency.
/// Projects in example/demo/test directories or with no recent activity get low scores.
pub(crate) fn compute_project_relevance(manifest_path: &Path) -> f32 {
    let path_str = manifest_path.to_string_lossy().to_lowercase();

    // Path pattern penalty: example/demo/test/tutorial directories -> 0.1x
    let path_score = if path_str.contains("/example")
        || path_str.contains("/demo")
        || path_str.contains("/test/") // Not /testing/ -- that's different
        || path_str.contains("/tests/")
        || path_str.contains("/tutorial")
        || path_str.contains("/template")
        || path_str.contains("/sample")
        || path_str.contains("/fixture")
        || path_str.contains("/benchmark")
        || path_str.contains("\\example")
        || path_str.contains("\\demo")
        || path_str.contains("\\test\\")
        || path_str.contains("\\tests\\")
        || path_str.contains("\\tutorial")
        || path_str.contains("\\template")
        || path_str.contains("\\sample")
        || path_str.contains("\\fixture")
        || path_str.contains("\\benchmark")
    {
        0.1
    } else {
        1.0
    };

    // Git recency: check for .git directory in parent chain
    let recency_score = compute_git_recency(manifest_path);

    (path_score * recency_score).clamp(0.0, 1.0)
}

/// Compute a recency score based on how recently the nearest git repository was modified.
/// Returns 1.0 for active repos (< 7 days), decaying to 0.1 for stale repos (> 90 days).
fn compute_git_recency(manifest_path: &Path) -> f32 {
    // Walk up from manifest to find .git directory
    let mut dir = manifest_path.parent();
    while let Some(d) = dir {
        let git_dir = d.join(".git");
        if git_dir.exists() {
            // Check HEAD modification time as proxy for last commit
            let head_file = git_dir.join("HEAD");
            if let Ok(metadata) = std::fs::metadata(&head_file) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        let days = elapsed.as_secs() as f32 / 86400.0;
                        return if days < 7.0 {
                            1.0
                        } else if days < 30.0 {
                            0.7
                        } else if days < 90.0 {
                            0.3
                        } else {
                            0.1
                        };
                    }
                }
            }
            return 0.5; // git exists but can't read metadata
        }
        dir = d.parent();
    }
    0.5 // no git found, neutral
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a string value from TOML content (basic implementation)
fn extract_toml_value(content: &str, key: &str) -> Option<String> {
    let pattern = format!("{key} = \"");
    if let Some(start) = content.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = content[value_start..].find('"') {
            return Some(content[value_start..value_start + end].to_string());
        }
    }
    None
}

/// True if a Cargo.toml dependency value declares a LOCAL source — an inline
/// table containing a `path` or `git` key (e.g. `{ path = "..." }` or
/// `{ git = "...", branch = "..." }`). Such deps resolve to the user's own
/// crates or vendored/forked code and have no crates.io registry presence, so
/// they must not be tracked as external dependencies. Plain version strings
/// (`"1.0"`) and registry tables (`{ version = "1", features = [...] }`) return
/// false.
/// Parse a `[target.<spec>.dependencies]` header, returning the target spec with
/// surrounding quotes stripped (e.g. `[target.'cfg(windows)'.dependencies]` ->
/// `cfg(windows)`, `[target.x86_64-pc-windows-msvc.dependencies]` -> the triple).
/// Returns None for any other header (incl. dev/build target sections, which are
/// not scanned for runtime advisories).
fn parse_target_deps_header(line: &str) -> Option<String> {
    let inner = line
        .strip_prefix("[target.")?
        .strip_suffix(".dependencies]")?;
    let spec = inner.trim().trim_matches(|c| c == '\'' || c == '"');
    if spec.is_empty() {
        None
    } else {
        Some(spec.to_string())
    }
}

fn is_local_cargo_dep(value: &str) -> bool {
    let v = value.trim();
    if !v.starts_with('{') {
        return false; // bare version string => registry dependency
    }
    toml_inline_has_key(v, "path") || toml_inline_has_key(v, "git")
}

/// Detect whether `key` appears as a KEY (token followed by `=`) inside an
/// inline TOML table string. Avoids matching the key inside a value such as a
/// feature name (`features = ["pathfinder"]`) by requiring a word boundary
/// before and an `=` immediately after (ignoring whitespace).
fn toml_inline_has_key(table: &str, key: &str) -> bool {
    let bytes = table.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = table[from..].find(key) {
        let pos = from + rel;
        let before_ok = pos == 0 || {
            let c = bytes[pos - 1] as char;
            !c.is_alphanumeric() && c != '_' && c != '-'
        };
        let after = table[pos + key.len()..].trim_start();
        if before_ok && after.starts_with('=') {
            return true;
        }
        from = pos + key.len();
    }
    false
}

/// True if an npm/package.json version spec points to a LOCAL or non-registry
/// source (`file:`, `link:`, `workspace:`, `portal:`, a git/github reference,
/// or a raw URL). These resolve to sibling/vendored packages with no npm
/// registry listing, so they must not be tracked as external dependencies.
/// Normal semver ranges (`^1.0.0`, `~2`, `*`, `latest`) and npm aliases
/// (`npm:pkg@1`) return false.
fn is_local_npm_spec(spec: &str) -> bool {
    let s = spec.trim();
    s.starts_with("file:")
        || s.starts_with("link:")
        || s.starts_with("workspace:")
        || s.starts_with("portal:")
        || s.starts_with("git+")
        || s.starts_with("git:")
        || s.starts_with("git@")
        || s.starts_with("github:")
        || s.contains("://")
}

/// Extract import/dependency names from source file imports.
/// Scans first 100 lines for language-specific import patterns.
/// Returns unique package/crate names found.
pub(crate) fn extract_imports_from_source(path: &Path) -> Vec<String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Only process known source files
    if !matches!(ext, "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go") {
        return Vec::new();
    }

    // Never force a cloud download to read import lines.
    if is_cloud_placeholder(path) {
        return Vec::new();
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut imports = HashSet::new();

    for line in content.lines().take(100) {
        let trimmed = line.trim();

        match ext {
            // Rust: use foo::bar, use foo::{...}
            "rs" => {
                if let Some(rest) = trimmed.strip_prefix("use ") {
                    if let Some(crate_name) = rest.split("::").next() {
                        let name = crate_name.trim_end_matches(';').trim();
                        // Skip std, self, super, crate
                        if !matches!(name, "std" | "self" | "super" | "crate" | "") {
                            imports.insert(name.to_string());
                        }
                    }
                }
            }
            // TypeScript/JavaScript: import ... from 'pkg', import 'pkg'
            "ts" | "tsx" | "js" | "jsx" => {
                if trimmed.starts_with("import ") {
                    // import { x } from 'pkg' or import x from 'pkg'
                    if let Some(from_part) = trimmed.split(" from ").nth(1) {
                        let pkg = from_part
                            .trim()
                            .trim_matches(|c| c == '\'' || c == '"' || c == ';');
                        if !pkg.starts_with('.') && !pkg.starts_with('/') && !pkg.is_empty() {
                            // Extract package name (handle scoped: @scope/pkg)
                            let name = if pkg.starts_with('@') {
                                pkg.splitn(3, '/').take(2).collect::<Vec<_>>().join("/")
                            } else {
                                pkg.split('/').next().unwrap_or(pkg).to_string()
                            };
                            imports.insert(name);
                        }
                    }
                    // import 'pkg' (side-effect import)
                    else if let Some(start) = trimmed.find('\'').or_else(|| trimmed.find('"')) {
                        let rest = &trimmed[start + 1..];
                        if let Some(end) = rest.find('\'').or_else(|| rest.find('"')) {
                            let pkg = &rest[..end];
                            if !pkg.starts_with('.') && !pkg.starts_with('/') && !pkg.is_empty() {
                                let name = if pkg.starts_with('@') {
                                    pkg.splitn(3, '/').take(2).collect::<Vec<_>>().join("/")
                                } else {
                                    pkg.split('/').next().unwrap_or(pkg).to_string()
                                };
                                imports.insert(name);
                            }
                        }
                    }
                }
            }
            // Python: from pkg import ..., import pkg
            "py" => {
                if let Some(rest) = trimmed.strip_prefix("from ") {
                    if let Some(pkg) = rest.split_whitespace().next() {
                        let top = pkg.split('.').next().unwrap_or(pkg);
                        if !top.is_empty() {
                            imports.insert(top.to_string());
                        }
                    }
                } else if let Some(rest) = trimmed.strip_prefix("import ") {
                    for part in rest.split(',') {
                        let pkg = part.split_whitespace().next().unwrap_or("");
                        let top = pkg.split('.').next().unwrap_or(pkg);
                        if !top.is_empty() {
                            imports.insert(top.to_string());
                        }
                    }
                }
            }
            // Go: import "pkg"
            "go" => {
                if trimmed.starts_with("import ") || trimmed.starts_with('"') {
                    if let Some(start) = trimmed.find('"') {
                        let rest = &trimmed[start + 1..];
                        if let Some(end) = rest.find('"') {
                            let pkg = &rest[..end];
                            // Extract last path segment as package name
                            if let Some(name) = pkg.rsplit('/').next() {
                                if !name.is_empty() {
                                    imports.insert(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    imports.into_iter().collect()
}

/// Signal indicating the user is actively learning a topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSignal {
    pub topic: String,
    pub source_dir: PathBuf,
}

/// Detect learning trajectory directories and tag their topics.
/// Directories named "learning", "tutorials", "courses", "study" indicate
/// the user is actively learning those technologies — boost, don't suppress.
pub(crate) fn detect_learning_directories(path: &Path) -> Vec<LearningSignal> {
    let learning_markers = [
        "learning",
        "tutorials",
        "courses",
        "study",
        "practice",
        "labs",
    ];
    let mut signals = Vec::new();

    let Ok(entries) = fs::read_dir(path) else {
        return signals;
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let dir_name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if learning_markers.iter().any(|m| dir_name.contains(m)) {
            // Scan subdirectories for technology topics
            if let Ok(sub_entries) = fs::read_dir(&entry_path) {
                for sub in sub_entries.flatten() {
                    if sub.path().is_dir() {
                        let topic = sub.file_name().to_string_lossy().to_lowercase();
                        if !topic.starts_with('.') && topic.len() > 1 {
                            signals.push(LearningSignal {
                                topic: topic.clone(),
                                source_dir: entry_path.clone(),
                            });
                        }
                    }
                }
            }

            // Also check for manifests to detect what's being learned
            let scanner = ProjectScanner::new();
            if let Ok(project_signals) = scanner.scan_directory(&entry_path) {
                for ps in project_signals {
                    for lang in &ps.languages {
                        signals.push(LearningSignal {
                            topic: lang.clone(),
                            source_dir: entry_path.clone(),
                        });
                    }
                    for fw in &ps.frameworks {
                        signals.push(LearningSignal {
                            topic: fw.clone(),
                            source_dir: entry_path.clone(),
                        });
                    }
                }
            }
        }
    }

    signals
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Dependency edge parsers (Step 1: reachability foundation)
    // ------------------------------------------------------------------

    #[test]
    fn test_parse_cargo_lock_edges() {
        let content = r#"
[[package]]
name = "app"
version = "0.1.0"
dependencies = [
 "serde",
 "tokio 1.35.0",
 "anyhow 1.0.75 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "serde"
version = "1.0.190"
dependencies = [
 "serde_derive 1.0.190",
]
"#;
        let edges = ProjectScanner::parse_cargo_lock_edges(content);

        // app -> serde (no version), app -> tokio 1.35.0, app -> anyhow 1.0.75
        assert!(edges.iter().any(|e| e.parent == "app"
            && e.child == "serde"
            && e.child_version.is_none()
            && e.scope == EdgeScope::Runtime));
        assert!(edges.iter().any(|e| e.parent == "app"
            && e.child == "tokio"
            && e.child_version.as_deref() == Some("1.35.0")));
        assert!(edges.iter().any(|e| e.parent == "app"
            && e.child == "anyhow"
            && e.child_version.as_deref() == Some("1.0.75")));
        // serde -> serde_derive 1.0.190
        assert!(edges.iter().any(|e| e.parent == "serde"
            && e.child == "serde_derive"
            && e.child_version.as_deref() == Some("1.0.190")));
        // Cargo edges are all runtime-scoped.
        assert!(edges.iter().all(|e| e.scope == EdgeScope::Runtime));
    }

    #[test]
    fn test_parse_cargo_lock_edges_inline_array() {
        let content = r#"
[[package]]
name = "app"
version = "0.1.0"
dependencies = ["once_cell", "log 0.4.20"]
"#;
        let edges = ProjectScanner::parse_cargo_lock_edges(content);
        assert!(edges
            .iter()
            .any(|e| e.parent == "app" && e.child == "once_cell"));
        assert!(edges.iter().any(|e| e.parent == "app"
            && e.child == "log"
            && e.child_version.as_deref() == Some("0.4.20")));
    }

    #[test]
    fn test_parse_package_lock_edges_v3_runtime_and_dev() {
        let content = r#"{
            "name": "app",
            "lockfileVersion": 3,
            "packages": {
                "": {
                    "name": "app",
                    "version": "1.0.0",
                    "dependencies": { "left-pad": "^1.3.0" },
                    "devDependencies": { "jest": "^29.0.0" }
                },
                "node_modules/left-pad": {
                    "version": "1.3.0",
                    "dependencies": { "tiny-dep": "^2.0.0" }
                },
                "node_modules/jest": {
                    "version": "29.7.0"
                }
            }
        }"#;
        let edges = ProjectScanner::parse_package_lock_edges(content);

        // root -> left-pad is runtime
        assert!(edges.iter().any(|e| e.parent == ROOT_PARENT
            && e.child == "left-pad"
            && e.scope == EdgeScope::Runtime));
        // root -> jest is dev
        assert!(edges
            .iter()
            .any(|e| e.parent == ROOT_PARENT && e.child == "jest" && e.scope == EdgeScope::Dev));
        // left-pad -> tiny-dep (transitive runtime)
        assert!(edges.iter().any(|e| e.parent == "left-pad"
            && e.child == "tiny-dep"
            && e.scope == EdgeScope::Runtime));
    }

    #[test]
    fn test_parse_package_lock_edges_v1_fallback() {
        let content = r#"{
            "name": "app",
            "lockfileVersion": 1,
            "dependencies": {
                "express": {
                    "version": "4.18.2",
                    "requires": { "body-parser": "1.20.1" }
                },
                "mocha": {
                    "version": "10.0.0",
                    "dev": true
                }
            }
        }"#;
        let edges = ProjectScanner::parse_package_lock_edges(content);

        assert!(edges.iter().any(|e| e.parent == ROOT_PARENT
            && e.child == "express"
            && e.scope == EdgeScope::Runtime));
        assert!(edges.iter().any(|e| e.parent == "express"
            && e.child == "body-parser"
            && e.scope == EdgeScope::Runtime));
        assert!(edges
            .iter()
            .any(|e| e.parent == ROOT_PARENT && e.child == "mocha" && e.scope == EdgeScope::Dev));
    }

    #[test]
    fn test_parse_pnpm_lock_edges_runtime_and_dev() {
        let content = r#"lockfileVersion: '6.0'

packages:

  /express@4.18.2:
    resolution: {integrity: sha512-aaa}
    dependencies:
      body-parser: 1.20.1
      cookie: 0.5.0
    devDependencies:
      supertest: 6.3.0

  /lodash@4.17.21:
    resolution: {integrity: sha512-bbb}
"#;
        let edges = ProjectScanner::parse_pnpm_lock_edges(content);

        assert!(edges.iter().any(|e| e.parent == "express"
            && e.child == "body-parser"
            && e.scope == EdgeScope::Runtime));
        assert!(edges.iter().any(|e| e.parent == "express"
            && e.child == "cookie"
            && e.scope == EdgeScope::Runtime));
        assert!(edges
            .iter()
            .any(|e| e.parent == "express" && e.child == "supertest" && e.scope == EdgeScope::Dev));
    }

    #[test]
    fn test_edge_parsers_are_robust_to_garbage() {
        assert!(ProjectScanner::parse_cargo_lock_edges("not a lockfile").is_empty());
        assert!(ProjectScanner::parse_package_lock_edges("{ broken json").is_empty());
        assert!(ProjectScanner::parse_pnpm_lock_edges("random: text").is_empty());
    }

    #[test]
    fn parse_cargo_toml_captures_target_gated_deps() {
        let content = r#"
[package]
name = "demo"

[dependencies]
serde = "1"

[target.'cfg(windows)'.dependencies]
winreg = "0.55"
windows-sys = { version = "0.59" }

[target.'cfg(not(windows))'.dependencies]
libc = "0.2"
"#;
        let scanner = ProjectScanner::new();
        let mut signal = ProjectSignal {
            manifest_type: ManifestType::CargoToml,
            manifest_path: PathBuf::from("Cargo.toml"),
            project_name: None,
            languages: vec!["rust".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };
        scanner.parse_cargo_toml(content, &mut signal);

        assert!(signal.dependencies.contains(&"serde".to_string()));
        // Target-gated deps are captured separately, not as plain dependencies.
        assert!(!signal.dependencies.contains(&"winreg".to_string()));
        let targets: std::collections::HashMap<String, String> =
            signal.target_dependencies.iter().cloned().collect();
        assert_eq!(
            targets.get("winreg").map(String::as_str),
            Some("cfg(windows)")
        );
        assert_eq!(
            targets.get("windows-sys").map(String::as_str),
            Some("cfg(windows)")
        );
        assert_eq!(
            targets.get("libc").map(String::as_str),
            Some("cfg(not(windows))")
        );
    }

    #[test]
    fn test_parse_cargo_toml() {
        let content = r#"
[package]
name = "my-project"
version = "0.1.0"
license = "MIT"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = "1.0"
axum = "0.7"

[dev-dependencies]
pretty_assertions = "1.0"
"#;

        let scanner = ProjectScanner::new();
        let mut signal = ProjectSignal {
            manifest_type: ManifestType::CargoToml,
            manifest_path: PathBuf::from("Cargo.toml"),
            project_name: None,
            languages: vec!["rust".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };

        scanner.parse_cargo_toml(content, &mut signal);

        assert_eq!(signal.project_name, Some("my-project".to_string()));
        assert!(signal.dependencies.contains(&"tokio".to_string()));
        assert!(signal.dependencies.contains(&"serde".to_string()));
        // tokio is a runtime, not a framework — should NOT be in frameworks
        assert!(!signal.frameworks.contains(&"tokio".to_string()));
        assert!(signal.frameworks.contains(&"axum".to_string()));
        assert_eq!(signal.project_license, Some("MIT".to_string()));
    }

    #[test]
    fn test_is_local_cargo_dep_classification() {
        // Local path / git deps -> true (skipped).
        assert!(is_local_cargo_dep(r#" { path = "fourda-macros" }"#));
        assert!(is_local_cargo_dep(r#"{ path="../shared" }"#));
        assert!(is_local_cargo_dep(r#" { git = "https://github.com/x/y" }"#));
        assert!(is_local_cargo_dep(
            r#" { git = "https://x/y", branch = "main" }"#
        ));
        // Registry deps -> false (kept).
        assert!(!is_local_cargo_dep(r#" "1.0""#));
        assert!(!is_local_cargo_dep(
            r#" { version = "1", features = ["full"] }"#
        ));
        // A feature literally named with "path" must NOT be misread as a path dep.
        assert!(!is_local_cargo_dep(
            r#" { version = "1", features = ["pathfinder"] }"#
        ));
    }

    #[test]
    fn test_parse_cargo_toml_skips_path_and_git_deps() {
        let content = r#"
[package]
name = "my-crate"

[dependencies]
serde = "1.0"
fourda-macros = { path = "fourda-macros" }
forked-lib = { git = "https://github.com/me/forked-lib" }
tokio = { version = "1", features = ["full"] }
"#;
        let scanner = ProjectScanner::new();
        let mut signal = ProjectSignal {
            manifest_type: ManifestType::CargoToml,
            manifest_path: PathBuf::from("Cargo.toml"),
            project_name: None,
            languages: vec!["rust".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };

        scanner.parse_cargo_toml(content, &mut signal);

        // Registry deps kept.
        assert!(signal.dependencies.contains(&"serde".to_string()));
        assert!(signal.dependencies.contains(&"tokio".to_string()));
        // Local path / git deps dropped — they have no crates.io presence.
        assert!(
            !signal.dependencies.contains(&"fourda-macros".to_string()),
            "path dep must be skipped"
        );
        assert!(
            !signal.dependencies.contains(&"forked-lib".to_string()),
            "git dep must be skipped"
        );
    }

    #[test]
    fn test_is_local_npm_spec_classification() {
        for s in [
            "file:../pkg",
            "link:../pkg",
            "workspace:*",
            "portal:../pkg",
            "git+https://github.com/x/y.git",
            "github:x/y",
            "https://example.com/x.tgz",
        ] {
            assert!(is_local_npm_spec(s), "{s} should be local");
        }
        for s in ["^1.0.0", "~2.3", "1.x", "*", "latest", "npm:aliased@1.0"] {
            assert!(!is_local_npm_spec(s), "{s} should be a registry spec");
        }
    }

    #[test]
    fn test_parse_package_json_skips_local_specs() {
        let content = r#"
{
  "name": "monorepo-app",
  "dependencies": {
    "react": "^18.0.0",
    "@scope/internal": "workspace:*",
    "local-tool": "file:../local-tool"
  },
  "devDependencies": {
    "typescript": "^5.0.0",
    "@scope/devkit": "link:../devkit"
  }
}
"#;
        let scanner = ProjectScanner::new();
        let mut signal = ProjectSignal {
            manifest_type: ManifestType::PackageJson,
            manifest_path: PathBuf::from("package.json"),
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

        scanner.parse_package_json(content, &mut signal);

        assert!(signal.dependencies.contains(&"react".to_string()));
        assert!(signal.dev_dependencies.contains(&"typescript".to_string()));
        assert!(!signal.dependencies.contains(&"@scope/internal".to_string()));
        assert!(!signal.dependencies.contains(&"local-tool".to_string()));
        assert!(!signal
            .dev_dependencies
            .contains(&"@scope/devkit".to_string()));
    }

    #[test]
    fn test_parse_package_json() {
        let content = r#"
{
  "name": "my-app",
  "license": "ISC",
  "dependencies": {
    "react": "^18.0.0",
    "next": "^14.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
  }
}
"#;

        let scanner = ProjectScanner::new();
        let mut signal = ProjectSignal {
            manifest_type: ManifestType::PackageJson,
            manifest_path: PathBuf::from("package.json"),
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

        scanner.parse_package_json(content, &mut signal);

        assert_eq!(signal.project_name, Some("my-app".to_string()));
        assert!(signal.dependencies.contains(&"react".to_string()));
        assert!(signal.frameworks.contains(&"react".to_string()));
        assert!(signal.frameworks.contains(&"next.js".to_string()));
        assert!(signal.languages.contains(&"typescript".to_string()));
        assert_eq!(signal.project_license, Some("ISC".to_string()));
    }

    fn p2_signal(mt: ManifestType, lang: &str) -> ProjectSignal {
        ProjectSignal {
            manifest_type: mt,
            manifest_path: PathBuf::from("manifest"),
            project_name: None,
            languages: vec![lang.to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        }
    }

    #[test]
    fn test_parse_composer_json() {
        let content = r#"{
  "name": "acme/billing",
  "license": "proprietary",
  "require": { "php": ">=8.1", "laravel/framework": "^10.0", "guzzlehttp/guzzle": "^7.5", "ext-json": "*" },
  "require-dev": { "phpunit/phpunit": "^10.0" }
}"#;
        let scanner = ProjectScanner::new();
        let mut signal = p2_signal(ManifestType::ComposerJson, "php");
        scanner.parse_composer_json(content, &mut signal);
        assert_eq!(signal.project_name, Some("acme/billing".to_string()));
        assert!(signal
            .dependencies
            .contains(&"laravel/framework".to_string()));
        assert!(signal
            .dependencies
            .contains(&"guzzlehttp/guzzle".to_string()));
        assert!(
            !signal.dependencies.contains(&"php".to_string()),
            "php meta excluded"
        );
        assert!(
            !signal.dependencies.iter().any(|d| d.starts_with("ext-")),
            "ext-* excluded"
        );
        assert!(signal.frameworks.contains(&"laravel".to_string()));
        assert!(signal
            .dev_dependencies
            .contains(&"phpunit/phpunit".to_string()));
    }

    #[test]
    fn test_parse_gemfile() {
        let content = r#"source "https://rubygems.org"
gem "rails", "~> 7.1"
gem 'pg'
gem "rspec-rails", group: :test
# a comment
"#;
        let scanner = ProjectScanner::new();
        let mut signal = p2_signal(ManifestType::Gemfile, "ruby");
        scanner.parse_gemfile(content, &mut signal);
        assert!(signal.dependencies.contains(&"rails".to_string()));
        assert!(signal.dependencies.contains(&"pg".to_string()));
        assert!(signal.frameworks.contains(&"rails".to_string()));
        assert!(signal.dev_dependencies.contains(&"rspec-rails".to_string()));
    }

    #[test]
    fn test_parse_pom_xml() {
        let content = r#"<project><name>acme-svc</name><dependencies>
  <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency>
  <dependency><groupId>junit</groupId><artifactId>junit</artifactId><scope>test</scope></dependency>
</dependencies></project>"#;
        let scanner = ProjectScanner::new();
        let mut signal = p2_signal(ManifestType::PomXml, "java");
        scanner.parse_pom_xml(content, &mut signal);
        assert_eq!(signal.project_name, Some("acme-svc".to_string()));
        assert!(signal
            .dependencies
            .contains(&"org.springframework.boot:spring-boot-starter-web".to_string()));
        assert!(signal.frameworks.contains(&"spring".to_string()));
        assert!(signal.dev_dependencies.contains(&"junit:junit".to_string()));
    }

    #[test]
    fn test_parse_build_gradle() {
        let content = r#"dependencies {
    implementation 'org.springframework.boot:spring-boot-starter:3.1.0'
    api("io.ktor:ktor-server-core:2.3.0")
    testImplementation "junit:junit:4.13.2"
}"#;
        let scanner = ProjectScanner::new();
        let mut signal = p2_signal(ManifestType::BuildGradle, "java");
        scanner.parse_build_gradle(content, &mut signal);
        assert!(signal
            .dependencies
            .contains(&"org.springframework.boot:spring-boot-starter".to_string()));
        assert!(signal
            .dependencies
            .contains(&"io.ktor:ktor-server-core".to_string()));
        assert!(signal.frameworks.contains(&"spring".to_string()));
        assert!(signal.frameworks.contains(&"ktor".to_string()));
        assert!(signal.dev_dependencies.contains(&"junit:junit".to_string()));
    }

    #[test]
    fn test_parse_csproj() {
        let content = r#"<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Microsoft.AspNetCore.Authentication.JwtBearer" Version="7.0.0" />
  </ItemGroup>
</Project>"#;
        let scanner = ProjectScanner::new();
        let mut signal = p2_signal(ManifestType::Csproj, "csharp");
        scanner.parse_csproj(content, &mut signal);
        assert!(signal.dependencies.contains(&"Newtonsoft.Json".to_string()));
        assert!(signal
            .dependencies
            .contains(&"Microsoft.AspNetCore.Authentication.JwtBearer".to_string()));
        assert!(signal.frameworks.contains(&"aspnet".to_string()));
    }

    #[test]
    fn test_parse_pubspec_yaml() {
        let content = r#"name: acme_app
dependencies:
  flutter:
    sdk: flutter
  http: ^1.1.0
  provider: ^6.0.5
dev_dependencies:
  flutter_test:
    sdk: flutter
  mockito: ^5.4.0
"#;
        let scanner = ProjectScanner::new();
        let mut signal = p2_signal(ManifestType::PubspecYaml, "dart");
        scanner.parse_pubspec_yaml(content, &mut signal);
        assert_eq!(signal.project_name, Some("acme_app".to_string()));
        assert!(signal.dependencies.contains(&"http".to_string()));
        assert!(signal.dependencies.contains(&"provider".to_string()));
        assert!(signal.dependencies.contains(&"flutter".to_string()));
        assert!(
            !signal.dependencies.contains(&"sdk".to_string()),
            "nested sdk: excluded"
        );
        assert!(signal.frameworks.contains(&"flutter".to_string()));
        assert!(signal.dev_dependencies.contains(&"mockito".to_string()));
    }

    #[test]
    fn test_serde_json_parse_keys() {
        let obj = r#"{ "react": "^18.0.0", "next": "^14.0.0" }"#;
        let parsed: serde_json::Value = serde_json::from_str(obj).unwrap();
        let keys: Vec<String> = parsed
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.to_string())
            .collect();
        assert!(keys.contains(&"react".to_string()));
        assert!(keys.contains(&"next".to_string()));
    }

    #[test]
    fn test_extract_imports_rust() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(
            &file,
            "use serde::Serialize;\nuse tokio::runtime;\nuse std::collections::HashMap;\nuse crate::db;\n",
        )
        .unwrap();
        let mut imports = extract_imports_from_source(&file);
        imports.sort();
        assert!(imports.contains(&"serde".to_string()));
        assert!(imports.contains(&"tokio".to_string()));
        // std, crate should be excluded
        assert!(!imports.contains(&"std".to_string()));
        assert!(!imports.contains(&"crate".to_string()));
    }

    #[test]
    fn test_extract_imports_typescript() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("index.ts");
        std::fs::write(
            &file,
            "import React from 'react';\nimport { useState } from 'react';\nimport { foo } from './local';\nimport '@tanstack/react-query';\n",
        )
        .unwrap();
        let mut imports = extract_imports_from_source(&file);
        imports.sort();
        assert!(imports.contains(&"react".to_string()));
        assert!(imports.contains(&"@tanstack/react-query".to_string()));
        // relative imports should be excluded
        assert!(!imports.iter().any(|i| i.contains("local")));
    }

    #[test]
    fn test_extract_imports_python() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.py");
        std::fs::write(
            &file,
            "from flask import Flask\nimport numpy as np\nimport os, sys\nfrom pandas.core import frame\n",
        )
        .unwrap();
        let mut imports = extract_imports_from_source(&file);
        imports.sort();
        assert!(imports.contains(&"flask".to_string()));
        assert!(imports.contains(&"numpy".to_string()));
        assert!(imports.contains(&"os".to_string()));
        assert!(imports.contains(&"pandas".to_string()));
    }

    #[test]
    fn test_extract_imports_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("readme.md");
        std::fs::write(&file, "# Hello\nimport something").unwrap();
        let imports = extract_imports_from_source(&file);
        assert!(imports.is_empty());
    }

    #[test]
    fn test_detect_learning_directories_empty() {
        let dir = tempfile::tempdir().unwrap();
        let signals = detect_learning_directories(dir.path());
        assert!(signals.is_empty());
    }

    #[test]
    fn test_detect_learning_directories_with_topics() {
        let dir = tempfile::tempdir().unwrap();
        let learning = dir.path().join("learning");
        std::fs::create_dir(&learning).unwrap();
        std::fs::create_dir(learning.join("rust")).unwrap();
        std::fs::create_dir(learning.join("python")).unwrap();
        // hidden dirs should be skipped
        std::fs::create_dir(learning.join(".hidden")).unwrap();

        let signals = detect_learning_directories(dir.path());
        let topics: Vec<&str> = signals.iter().map(|s| s.topic.as_str()).collect();
        assert!(topics.contains(&"rust"));
        assert!(topics.contains(&"python"));
        assert!(!topics.contains(&".hidden"));
    }

    #[test]
    fn test_parse_cargo_toml_workspace_deps() {
        let content = r#"
[workspace]
members = ["crates/*"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = "1"
anyhow = "1.0"

[dependencies]
axum = "0.7"
"#;

        let scanner = ProjectScanner::new();
        let mut signal = ProjectSignal {
            manifest_type: ManifestType::CargoToml,
            manifest_path: PathBuf::from("Cargo.toml"),
            project_name: None,
            languages: vec!["rust".to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            target_dependencies: Vec::new(),
            detected_at: String::new(),
            project_license: None,
            project_relevance: 1.0,
        };

        scanner.parse_cargo_toml(content, &mut signal);

        // workspace.dependencies should be parsed as regular dependencies
        assert!(signal.dependencies.contains(&"serde".to_string()));
        assert!(signal.dependencies.contains(&"tokio".to_string()));
        assert!(signal.dependencies.contains(&"anyhow".to_string()));
        // Regular dependencies should also be present
        assert!(signal.dependencies.contains(&"axum".to_string()));
        // Frameworks should be detected from workspace deps too
        // tokio is a runtime, not a framework — only axum should be detected
        assert!(!signal.frameworks.contains(&"tokio".to_string()));
        assert!(signal.frameworks.contains(&"axum".to_string()));
    }

    #[test]
    fn test_check_manifests_merges_imports() {
        let dir = tempfile::tempdir().unwrap();

        // Create a Cargo.toml with one dependency
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"

[dependencies]
serde = "1.0"
"#,
        )
        .unwrap();

        // Create a source file that imports something NOT in the manifest
        std::fs::write(
            dir.path().join("main.rs"),
            "use tracing::info;\nuse serde::Serialize;\n",
        )
        .unwrap();

        // Also create src/ with another import
        let src = dir.path().join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(
            src.join("lib.rs"),
            "use anyhow::Result;\nuse serde::Deserialize;\n",
        )
        .unwrap();

        let scanner = ProjectScanner::new();
        let mut signals = Vec::new();
        scanner.check_manifests(dir.path(), &mut signals).unwrap();

        assert_eq!(signals.len(), 1);
        let signal = &signals[0];
        assert_eq!(signal.project_name, Some("test-project".to_string()));
        // serde was in manifest — should be present
        assert!(signal.dependencies.contains(&"serde".to_string()));
        // tracing and anyhow were imported but not in manifest — should be merged
        assert!(
            signal.dependencies.contains(&"tracing".to_string()),
            "tracing should be merged from main.rs imports"
        );
        assert!(
            signal.dependencies.contains(&"anyhow".to_string()),
            "anyhow should be merged from src/lib.rs imports"
        );
        // serde should NOT be duplicated (already in deps from manifest)
        assert_eq!(
            signal.dependencies.iter().filter(|d| *d == "serde").count(),
            1,
            "serde should not be duplicated"
        );
    }

    #[test]
    fn test_parse_cargo_lock() {
        let content = "# This file is automatically @generated by Cargo.\n\
            # It is not intended for manual editing.\n\
            version = 4\n\
            \n\
            [[package]]\n\
            name = \"serde\"\n\
            version = \"1.0.204\"\n\
            source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
            \n\
            [[package]]\n\
            name = \"serde_derive\"\n\
            version = \"1.0.204\"\n\
            source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
            \n\
            [[package]]\n\
            name = \"proc-macro2\"\n\
            version = \"1.0.86\"\n\
            source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";

        let packages = ProjectScanner::parse_cargo_lock(content);
        assert_eq!(packages.len(), 3);
        assert!(packages.iter().any(|(n, v)| n == "serde" && v == "1.0.204"));
        assert!(packages
            .iter()
            .any(|(n, v)| n == "serde_derive" && v == "1.0.204"));
        assert!(packages
            .iter()
            .any(|(n, v)| n == "proc-macro2" && v == "1.0.86"));
    }

    #[test]
    fn test_parse_cargo_lock_empty() {
        let content = "# This file is automatically @generated by Cargo.\nversion = 4\n";
        let packages = ProjectScanner::parse_cargo_lock(content);
        assert!(packages.is_empty());
    }

    #[test]
    fn test_parse_package_lock_json_v3() {
        let content = serde_json::json!({
            "name": "my-app",
            "version": "1.0.0",
            "lockfileVersion": 3,
            "packages": {
                "": { "name": "my-app", "version": "1.0.0" },
                "node_modules/lodash": { "version": "4.17.21" },
                "node_modules/@babel/core": { "version": "7.24.0" },
                "node_modules/@babel/core/node_modules/semver": { "version": "6.3.1" }
            }
        })
        .to_string();

        let packages = ProjectScanner::parse_package_lock_json(&content);
        // Root "" entry and nested node_modules should be excluded
        assert_eq!(packages.len(), 2);
        assert!(packages
            .iter()
            .any(|(n, v)| n == "lodash" && v == "4.17.21"));
        assert!(packages
            .iter()
            .any(|(n, v)| n == "@babel/core" && v == "7.24.0"));
    }

    #[test]
    fn test_parse_package_lock_json_v1() {
        let content = serde_json::json!({
            "name": "old-app",
            "version": "1.0.0",
            "lockfileVersion": 1,
            "dependencies": {
                "express": { "version": "4.18.2" },
                "body-parser": { "version": "1.20.2" }
            }
        })
        .to_string();

        let packages = ProjectScanner::parse_package_lock_json(&content);
        assert_eq!(packages.len(), 2);
        assert!(packages
            .iter()
            .any(|(n, v)| n == "express" && v == "4.18.2"));
        assert!(packages
            .iter()
            .any(|(n, v)| n == "body-parser" && v == "1.20.2"));
    }

    #[test]
    fn test_parse_package_lock_json_invalid() {
        let packages = ProjectScanner::parse_package_lock_json("not valid json");
        assert!(packages.is_empty());
    }

    // ─── Project relevance scoring ──────────────────────────────────

    #[test]
    fn test_relevance_example_dirs_get_low_score() {
        // Forward-slash paths (Unix style)
        let path = PathBuf::from("/home/user/vercel-workflow/example/package.json");
        let score = compute_project_relevance(&path);
        assert!(
            score <= 0.1,
            "example dir should get low relevance, got {score}"
        );

        let path = PathBuf::from("/projects/demo/my-app/Cargo.toml");
        let score = compute_project_relevance(&path);
        assert!(
            score <= 0.1,
            "demo dir should get low relevance, got {score}"
        );

        let path = PathBuf::from("/projects/tutorial/react-basics/package.json");
        let score = compute_project_relevance(&path);
        assert!(
            score <= 0.1,
            "tutorial dir should get low relevance, got {score}"
        );
    }

    #[test]
    fn test_relevance_backslash_paths_also_penalized() {
        // Windows-style backslash paths
        let path = PathBuf::from("C:\\Users\\dev\\example\\my-app\\package.json");
        let score = compute_project_relevance(&path);
        assert!(
            score <= 0.1,
            "Windows example dir should get low relevance, got {score}"
        );

        let path = PathBuf::from("D:\\projects\\test\\fixture\\package.json");
        let score = compute_project_relevance(&path);
        assert!(
            score <= 0.1,
            "Windows test+fixture dir should get low relevance, got {score}"
        );
    }

    #[test]
    fn test_relevance_production_paths_not_penalized() {
        // Normal production project paths should not be penalized by path score
        let path = PathBuf::from("/home/user/my-production-app/Cargo.toml");
        let score = compute_project_relevance(&path);
        // Path score should be 1.0; git recency may vary (0.5 if no git found)
        assert!(
            score >= 0.3,
            "production path should not be penalized, got {score}"
        );
    }

    #[test]
    fn test_relevance_testing_not_penalized() {
        // /testing/ should NOT be penalized (only /test/ and /tests/)
        let path = PathBuf::from("/home/user/testing-framework/Cargo.toml");
        let score = compute_project_relevance(&path);
        assert!(
            score >= 0.3,
            "testing-framework should not be penalized, got {score}"
        );
    }

    #[test]
    fn test_relevance_all_penalty_patterns() {
        let penalty_patterns = vec![
            "/example/",
            "/demo/",
            "/test/nested/",
            "/tests/nested/",
            "/tutorial/",
            "/template/",
            "/sample/",
            "/fixture/",
            "/benchmark/",
        ];
        for pattern in penalty_patterns {
            let path = PathBuf::from(format!("/projects{pattern}Cargo.toml"));
            let score = compute_project_relevance(&path);
            assert!(
                score <= 0.1,
                "pattern '{pattern}' should get low relevance, got {score}"
            );
        }
    }

    // ─── pnpm-lock.yaml parsing ────────────────────────────────────

    #[test]
    fn test_parse_pnpm_lock_v9() {
        let content = "\
lockfileVersion: '9.0'

packages:

  lodash@4.17.21:
    resolution: {integrity: sha512-abc}

  '@babel/core@7.24.0':
    resolution: {integrity: sha512-def}

  vite@5.2.0:
    resolution: {integrity: sha512-ghi}
";
        let packages = ProjectScanner::parse_pnpm_lock_yaml(content);
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "lodash" && v == "4.17.21"),
            "should parse lodash, got: {packages:?}"
        );
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "@babel/core" && v == "7.24.0"),
            "should parse scoped package, got: {packages:?}"
        );
        assert!(
            packages.iter().any(|(n, v)| n == "vite" && v == "5.2.0"),
            "should parse vite, got: {packages:?}"
        );
    }

    #[test]
    fn test_parse_pnpm_lock_v5() {
        let content = "\
lockfileVersion: 5.4

packages:

  /lodash/4.17.21:
    resolution: {integrity: sha512-abc}

  /@types/node/20.11.0:
    resolution: {integrity: sha512-def}
";
        let packages = ProjectScanner::parse_pnpm_lock_yaml(content);
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "lodash" && v == "4.17.21"),
            "should parse v5 lodash, got: {packages:?}"
        );
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "@types/node" && v == "20.11.0"),
            "should parse v5 scoped package, got: {packages:?}"
        );
    }

    #[test]
    fn test_parse_pnpm_lock_empty() {
        let content = "lockfileVersion: '9.0'\n\nimporters: {}\n";
        let packages = ProjectScanner::parse_pnpm_lock_yaml(content);
        assert!(packages.is_empty());
    }

    #[test]
    fn test_parse_pnpm_lock_version_with_peer_suffix() {
        let content = "\
lockfileVersion: '9.0'

packages:

  react-dom@18.2.0:
    version: 18.2.0(react@18.2.0)
";
        let packages = ProjectScanner::parse_pnpm_lock_yaml(content);
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "react-dom" && v == "18.2.0"),
            "should strip peer suffix, got: {packages:?}"
        );
    }

    // ─── yarn.lock parsing ─────────────────────────────────────────

    #[test]
    fn test_parse_yarn_lock() {
        let content = r#"# yarn lockance v1

lodash@^4.17.20:
  version "4.17.21"
  resolved "https://registry.yarnpkg.com/lodash/-/lodash-4.17.21.tgz"

"@babel/core@^7.24.0":
  version "7.24.0"
  resolved "https://registry.yarnpkg.com/@babel/core/-/core-7.24.0.tgz"
"#;
        let packages = ProjectScanner::parse_yarn_lock(content);
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "lodash" && v == "4.17.21"),
            "should parse lodash, got: {packages:?}"
        );
        assert!(
            packages
                .iter()
                .any(|(n, v)| n == "@babel/core" && v == "7.24.0"),
            "should parse scoped package, got: {packages:?}"
        );
    }

    #[test]
    fn test_parse_yarn_lock_empty() {
        let content = "# yarn lockfile v1\n\n";
        let packages = ProjectScanner::parse_yarn_lock(content);
        assert!(packages.is_empty());
    }

    // ─── poetry.lock parsing ──────────────────────────────────────

    #[test]
    fn test_parse_poetry_lock() {
        let content = r#"
[[package]]
name = "requests"
version = "2.31.0"
description = "Python HTTP for Humans."

[[package]]
name = "urllib3"
version = "2.1.0"
description = "HTTP library"

[[package]]
name = "certifi"
version = "2024.2.2"
"#;
        let packages = ProjectScanner::parse_poetry_lock(content);
        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&("requests".to_string(), "2.31.0".to_string())));
        assert!(packages.contains(&("urllib3".to_string(), "2.1.0".to_string())));
        assert!(packages.contains(&("certifi".to_string(), "2024.2.2".to_string())));
    }

    #[test]
    fn test_parse_poetry_lock_empty() {
        assert!(ProjectScanner::parse_poetry_lock("").is_empty());
    }

    // ─── requirements.txt exact-pin parsing ───────────────────────

    #[test]
    fn test_parse_requirements_txt_pins() {
        let content = "\
# Reference stack
torch==2.3.0
transformers==4.41.0
pillow==10.3.0  # inline comment
fastapi[all]==0.111.0
uvicorn==0.29.0 ; python_version >= '3.8'
numpy>=1.26.0
pandas~=2.2
flask
-r other.txt
weird===1.0.0
";
        let pins = ProjectScanner::parse_requirements_txt_pins(content);
        // Exact pins captured (name + version), extras/markers/comments stripped.
        assert!(pins.contains(&("torch".to_string(), "2.3.0".to_string())));
        assert!(pins.contains(&("transformers".to_string(), "4.41.0".to_string())));
        assert!(pins.contains(&("pillow".to_string(), "10.3.0".to_string())));
        assert!(pins.contains(&("fastapi".to_string(), "0.111.0".to_string())));
        assert!(pins.contains(&("uvicorn".to_string(), "0.29.0".to_string())));
        // Non-exact specifiers, bare names, options, and `===` are NOT captured.
        for (name, _) in &pins {
            assert!(
                !["numpy", "pandas", "flask", "weird"].contains(&name.as_str()),
                "non-exact/option/arbitrary line wrongly captured: {name}"
            );
        }
        assert_eq!(pins.len(), 5);
    }

    #[test]
    fn test_parse_requirements_txt_pins_empty() {
        assert!(ProjectScanner::parse_requirements_txt_pins("").is_empty());
        assert!(ProjectScanner::parse_requirements_txt_pins("# only a comment\n-e .\n").is_empty());
    }

    // ─── go.sum parsing ───────────────────────────────────────────

    #[test]
    fn test_parse_go_sum() {
        let content = r#"golang.org/x/net v0.17.0 h1:hash123=
golang.org/x/net v0.17.0/go.mod h1:hash456=
golang.org/x/crypto v0.14.0 h1:hash789=
golang.org/x/crypto v0.14.0/go.mod h1:hashabc=
github.com/gin-gonic/gin v1.9.1 h1:hashdef=
github.com/gin-gonic/gin v1.9.1/go.mod h1:hashghi=
"#;
        let packages = ProjectScanner::parse_go_sum(content);
        assert_eq!(packages.len(), 3, "should dedup module+go.mod entries");
        assert!(packages.contains(&("golang.org/x/net".to_string(), "0.17.0".to_string())));
        assert!(packages.contains(&("golang.org/x/crypto".to_string(), "0.14.0".to_string())));
        assert!(packages.contains(&("github.com/gin-gonic/gin".to_string(), "1.9.1".to_string())));
    }

    #[test]
    fn test_parse_go_sum_empty() {
        assert!(ProjectScanner::parse_go_sum("").is_empty());
    }

    #[test]
    fn test_parse_go_sum_pseudo_version() {
        let content = "golang.org/x/sys v0.0.0-20220520151302-bc2c85ada10a h1:hash=\n";
        let packages = ProjectScanner::parse_go_sum(content);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].0, "golang.org/x/sys");
        assert_eq!(packages[0].1, "0.0.0-20220520151302-bc2c85ada10a");
    }

    // ─── Gemfile.lock parsing ─────────────────────────────────────

    #[test]
    fn test_parse_gemfile_lock() {
        let content = r#"GEM
  remote: https://rubygems.org/
  specs:
    actioncable (7.1.3)
      actionpack (= 7.1.3)
    actionpack (7.1.3)
      rack (~> 3.0)
    rack (3.0.8)
    rails (7.1.3)
      actioncable (= 7.1.3)

PLATFORMS
  ruby

DEPENDENCIES
  rails (~> 7.1)

BUNDLED WITH
   2.5.6
"#;
        let packages = ProjectScanner::parse_gemfile_lock(content);
        assert_eq!(packages.len(), 4);
        assert!(packages.contains(&("actioncable".to_string(), "7.1.3".to_string())));
        assert!(packages.contains(&("actionpack".to_string(), "7.1.3".to_string())));
        assert!(packages.contains(&("rack".to_string(), "3.0.8".to_string())));
        assert!(packages.contains(&("rails".to_string(), "7.1.3".to_string())));
    }

    #[test]
    fn test_parse_gemfile_lock_empty() {
        assert!(ProjectScanner::parse_gemfile_lock("").is_empty());
    }

    #[test]
    fn test_parse_gemfile_lock_skips_dependency_constraints() {
        let content = r#"GEM
  specs:
    nokogiri (1.16.2)
      racc (~> 1.4)
    racc (1.7.3)
"#;
        let packages = ProjectScanner::parse_gemfile_lock(content);
        // Both nokogiri and racc should be found (the (~> 1.4) constraint line
        // for racc as a sub-dep should NOT be parsed as a package)
        assert_eq!(packages.len(), 2);
        assert!(packages.contains(&("nokogiri".to_string(), "1.16.2".to_string())));
        assert!(packages.contains(&("racc".to_string(), "1.7.3".to_string())));
    }

    #[test]
    fn test_parse_composer_lock() {
        let content = r#"{
            "packages": [
                {"name": "monolog/monolog", "version": "3.5.0"},
                {"name": "symfony/console", "version": "v6.4.3"}
            ],
            "packages-dev": [
                {"name": "phpunit/phpunit", "version": "10.5.9"}
            ]
        }"#;
        let packages = ProjectScanner::parse_composer_lock(content);
        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&("monolog/monolog".to_string(), "3.5.0".to_string())));
        assert!(packages.contains(&("symfony/console".to_string(), "6.4.3".to_string())));
        assert!(packages.contains(&("phpunit/phpunit".to_string(), "10.5.9".to_string())));
    }

    #[test]
    fn test_parse_composer_lock_empty() {
        assert!(ProjectScanner::parse_composer_lock("{}").is_empty());
        assert!(ProjectScanner::parse_composer_lock("invalid").is_empty());
    }

    // ─── Path exclusion tests ─────────────────────────────────────

    #[test]
    fn test_excluded_path_claude_worktrees() {
        // Unix-style
        assert!(ProjectScanner::is_excluded_path(Path::new(
            "/home/user/project/.claude/worktrees/agent-abc123/src"
        )));
        // Windows-style
        assert!(ProjectScanner::is_excluded_path(Path::new(
            r"D:\4DA\.claude\worktrees\agent-abc123"
        )));
    }

    #[test]
    fn test_excluded_path_git_worktrees() {
        assert!(ProjectScanner::is_excluded_path(Path::new(
            "/home/user/project/.git/worktrees/feature-branch"
        )));
        assert!(ProjectScanner::is_excluded_path(Path::new(
            r"D:\4DA\.git\worktrees\feature-branch"
        )));
    }

    #[test]
    fn test_excluded_path_normal_dirs_not_excluded() {
        assert!(!ProjectScanner::is_excluded_path(Path::new(
            "/home/user/project/src"
        )));
        assert!(!ProjectScanner::is_excluded_path(Path::new(
            r"D:\4DA\src-tauri\src"
        )));
        assert!(!ProjectScanner::is_excluded_path(Path::new(
            "/home/user/worktrees/my-project"
        )));
        // .claude directory itself is fine — only .claude/worktrees/ is excluded
        assert!(!ProjectScanner::is_excluded_path(Path::new(
            "/home/user/project/.claude/plans"
        )));
    }

    #[test]
    fn test_scan_skips_claude_worktree_manifests() {
        let dir = tempfile::tempdir().unwrap();

        // Create a real project at root
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"real-project\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Create a fake worktree project that should be skipped
        let worktree_dir = dir
            .path()
            .join(".claude")
            .join("worktrees")
            .join("agent-abc123");
        std::fs::create_dir_all(&worktree_dir).unwrap();
        std::fs::write(
            worktree_dir.join("package.json"),
            r#"{"name": "worktree-copy", "dependencies": {"express": "4.0"}}"#,
        )
        .unwrap();

        let scanner = ProjectScanner::new();
        let signals = scanner.scan_directory(dir.path()).unwrap();

        // Should find the real project but NOT the worktree copy
        assert_eq!(
            signals.len(),
            1,
            "Should only find 1 project, not the worktree copy"
        );
        assert_eq!(
            signals[0].manifest_type,
            ManifestType::CargoToml,
            "Should find the real Cargo.toml, not the worktree package.json"
        );
    }
}

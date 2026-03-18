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
    pub detected_at: String,
}

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
            max_depth: 5,
            skip_dirs,
        }
    }

    /// Maximum signals to collect (prevents OOM on huge repos)
    const MAX_SIGNALS: usize = 500;

    /// Maximum path length to process (Windows MAX_PATH guard)
    #[cfg(target_os = "windows")]
    const MAX_PATH_LEN: usize = 260;
    #[cfg(not(target_os = "windows"))]
    const MAX_PATH_LEN: usize = 4096;

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

            // Scan source files in the manifest directory
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if files_scanned >= MAX_SOURCE_FILES {
                        break;
                    }
                    let path = entry.path();
                    if path.is_file() {
                        let extracted = extract_imports_from_source(&path);
                        if !extracted.is_empty() {
                            import_deps.extend(extracted);
                            files_scanned += 1;
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
                            let extracted = extract_imports_from_source(&path);
                            if !extracted.is_empty() {
                                import_deps.extend(extracted);
                                files_scanned += 1;
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
        let content = fs::read_to_string(path).ok()?;

        let mut signal = ProjectSignal {
            manifest_type,
            manifest_path: path.to_path_buf(),
            project_name: None,
            languages: vec![manifest_type.language().to_string()],
            frameworks: Vec::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
            detected_at: chrono::Utc::now().to_rfc3339(),
        };

        match manifest_type {
            ManifestType::CargoToml => self.parse_cargo_toml(&content, &mut signal),
            ManifestType::PackageJson => self.parse_package_json(&content, &mut signal),
            ManifestType::PyprojectToml => self.parse_pyproject_toml(&content, &mut signal),
            ManifestType::RequirementsTxt => self.parse_requirements_txt(&content, &mut signal),
            ManifestType::GoMod => self.parse_go_mod(&content, &mut signal),
            _ => {} // Basic detection for others
        }

        Some(signal)
    }

    fn parse_cargo_toml(&self, content: &str, signal: &mut ProjectSignal) {
        // Parse with toml crate if available, otherwise use regex
        // Extract package name
        if let Some(name) = extract_toml_value(content, "name") {
            signal.project_name = Some(name);
        }

        // Extract dependencies
        let mut in_deps = false;
        let mut in_dev_deps = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == "[dependencies]" {
                in_deps = true;
                in_dev_deps = false;
                continue;
            } else if trimmed == "[dev-dependencies]" {
                in_deps = false;
                in_dev_deps = true;
                continue;
            } else if trimmed.starts_with('[') {
                in_deps = false;
                in_dev_deps = false;
                continue;
            }

            if (in_deps || in_dev_deps) && !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some(dep_name) = trimmed.split('=').next() {
                    let dep = dep_name.trim().to_string();
                    if !dep.is_empty() {
                        if in_dev_deps {
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
        let frameworks = [
            ("tauri", "tauri"),
            ("actix", "actix-web"),
            ("axum", "axum"),
            ("rocket", "rocket"),
            ("warp", "warp"),
            ("tokio", "tokio"),
            ("async-std", "async-std"),
            ("sqlx", "sqlx"),
            ("diesel", "diesel"),
            ("serde", "serde"),
            ("tonic", "grpc/tonic"),
        ];

        let dep_lower = dep.to_lowercase();
        for (pattern, framework) in frameworks {
            let matches = dep_lower == pattern || dep_lower.starts_with(&format!("{}-", pattern));
            if matches && !signal.frameworks.contains(&framework.to_string()) {
                signal.frameworks.push(framework.to_string());
            }
        }
    }

    fn parse_package_json(&self, content: &str, signal: &mut ProjectSignal) {
        // Basic JSON parsing without full serde_json
        // Extract name
        if let Some(name) = extract_json_string(content, "name") {
            signal.project_name = Some(name);
        }

        // Check for TypeScript
        if (content.contains("\"typescript\"") || content.contains("\"@types/"))
            && !signal.languages.contains(&"typescript".to_string())
        {
            signal.languages.push("typescript".to_string());
        }

        // Extract dependencies
        if let Some(deps_section) = extract_json_object(content, "dependencies") {
            for dep in extract_json_keys(&deps_section) {
                signal.dependencies.push(dep.clone());
                self.detect_js_framework(&dep, signal);
            }
        }

        if let Some(dev_deps_section) = extract_json_object(content, "devDependencies") {
            for dep in extract_json_keys(&dev_deps_section) {
                signal.dev_dependencies.push(dep);
            }
        }
    }

    fn detect_js_framework(&self, dep: &str, signal: &mut ProjectSignal) {
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
            ("prisma", "prisma"),
            ("drizzle", "drizzle"),
            ("vite", "vite"),
            ("webpack", "webpack"),
            ("tailwindcss", "tailwind"),
            ("electron", "electron"),
        ];

        let dep_lower = dep.to_lowercase();
        for (pattern, framework) in frameworks {
            let matches = dep_lower == pattern
                || dep_lower.starts_with(&format!("{}-", pattern))
                || dep_lower.ends_with(&format!("/{}", pattern));
            if matches && !signal.frameworks.contains(&framework.to_string()) {
                signal.frameworks.push(framework.to_string());
            }
        }
    }

    fn parse_pyproject_toml(&self, content: &str, signal: &mut ProjectSignal) {
        // Extract project name
        if let Some(name) = extract_toml_value(content, "name") {
            signal.project_name = Some(name);
        }

        // Look for common frameworks
        let python_frameworks = [
            ("django", "django"),
            ("flask", "flask"),
            ("fastapi", "fastapi"),
            ("pandas", "pandas"),
            ("numpy", "numpy"),
            ("tensorflow", "tensorflow"),
            ("torch", "pytorch"),
            ("scikit-learn", "scikit-learn"),
            ("sqlalchemy", "sqlalchemy"),
            ("celery", "celery"),
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
                let frameworks = [
                    ("django", "django"),
                    ("flask", "flask"),
                    ("fastapi", "fastapi"),
                    ("pandas", "pandas"),
                    ("numpy", "numpy"),
                    ("tensorflow", "tensorflow"),
                    ("torch", "pytorch"),
                    ("scikit-learn", "scikit-learn"),
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

    fn parse_go_mod(&self, content: &str, signal: &mut ProjectSignal) {
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
                    let frameworks = [
                        ("gin-gonic", "gin"),
                        ("echo", "echo"),
                        ("fiber", "fiber"),
                        ("gorm", "gorm"),
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
}

impl Default for ProjectScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract a string value from TOML content (basic implementation)
fn extract_toml_value(content: &str, key: &str) -> Option<String> {
    let pattern = format!("{} = \"", key);
    if let Some(start) = content.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = content[value_start..].find('"') {
            return Some(content[value_start..value_start + end].to_string());
        }
    }
    None
}

/// Extract a string value from JSON content (basic implementation)
fn extract_json_string(content: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(key_pos) = content.find(&pattern) {
        let after_key = &content[key_pos + pattern.len()..];
        // Find the colon and opening quote
        if let Some(colon_pos) = after_key.find(':') {
            let after_colon = &after_key[colon_pos + 1..];
            // Find opening quote
            if let Some(quote_start) = after_colon.find('"') {
                let value_start = quote_start + 1;
                // Find closing quote
                if let Some(quote_end) = after_colon[value_start..].find('"') {
                    return Some(after_colon[value_start..value_start + quote_end].to_string());
                }
            }
        }
    }
    None
}

/// Extract an object section from JSON content (basic implementation)
fn extract_json_object(content: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(key_pos) = content.find(&pattern) {
        let after_key = &content[key_pos + pattern.len()..];
        if let Some(brace_start) = after_key.find('{') {
            let obj_start = brace_start;
            let mut depth = 1;
            let mut obj_end = obj_start + 1;

            for (i, c) in after_key[obj_start + 1..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            obj_end = obj_start + 1 + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            return Some(after_key[obj_start..obj_end].to_string());
        }
    }
    None
}

/// Extract keys from a JSON object string (basic implementation)
fn extract_json_keys(obj: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut in_string = false;
    let mut key_start = None;

    for (i, c) in obj.char_indices() {
        match c {
            '"' if !in_string => {
                in_string = true;
                key_start = Some(i + 1);
            }
            '"' if in_string => {
                if let Some(start) = key_start {
                    let key = &obj[start..i];
                    // Only add if the next non-whitespace is a colon (it's a key, not a value)
                    let rest = &obj[i + 1..];
                    if rest.trim_start().starts_with(':') {
                        keys.push(key.to_string());
                    }
                }
                in_string = false;
                key_start = None;
            }
            _ => {}
        }
    }

    keys
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
                        let pkg = part.trim().split_whitespace().next().unwrap_or("");
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

    #[test]
    fn test_parse_cargo_toml() {
        let content = r#"
[package]
name = "my-project"
version = "0.1.0"

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
            detected_at: String::new(),
        };

        scanner.parse_cargo_toml(content, &mut signal);

        assert_eq!(signal.project_name, Some("my-project".to_string()));
        assert!(signal.dependencies.contains(&"tokio".to_string()));
        assert!(signal.dependencies.contains(&"serde".to_string()));
        assert!(signal.frameworks.contains(&"tokio".to_string()));
        assert!(signal.frameworks.contains(&"axum".to_string()));
    }

    #[test]
    fn test_parse_package_json() {
        let content = r#"
{
  "name": "my-app",
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
            detected_at: String::new(),
        };

        scanner.parse_package_json(content, &mut signal);

        assert_eq!(signal.project_name, Some("my-app".to_string()));
        assert!(signal.dependencies.contains(&"react".to_string()));
        assert!(signal.frameworks.contains(&"react".to_string()));
        assert!(signal.frameworks.contains(&"next.js".to_string()));
        assert!(signal.languages.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_extract_json_keys() {
        let obj = r#"{ "react": "^18.0.0", "next": "^14.0.0" }"#;
        let keys = extract_json_keys(obj);
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
}

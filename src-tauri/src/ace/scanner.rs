//! Project Manifest Scanner
//!
//! Scans directories for project manifests and extracts technology stack.
//! Supports: package.json, Cargo.toml, pyproject.toml, go.mod, and more.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

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
    pub fn scan_directory(&self, path: &Path) -> Result<Vec<ProjectSignal>, String> {
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
    ) -> Result<(), String> {
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
                let _ = self.scan_recursive(&entry_path, depth + 1, signals, visited);
            }
        }

        Ok(())
    }

    fn check_manifests(&self, dir: &Path, signals: &mut Vec<ProjectSignal>) -> Result<(), String> {
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

        for (pattern, framework) in frameworks {
            if dep.to_lowercase().contains(pattern)
                && !signal.frameworks.contains(&framework.to_string())
            {
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

        for (pattern, framework) in frameworks {
            if dep.to_lowercase().contains(pattern)
                && !signal.frameworks.contains(&framework.to_string())
            {
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
}

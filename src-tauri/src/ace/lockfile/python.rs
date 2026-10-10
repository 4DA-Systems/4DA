// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Python: `poetry.lock`, `uv.lock`, `pdm.lock` (TOML `[[package]]`
//! tables), `Pipfile.lock` (JSON) and pinned requirements files.
//!
//! Before 2026-10-10 only `poetry.lock` and a file named exactly
//! `requirements.txt` were read; uv.lock, Pipfile.lock, pdm.lock,
//! `requirements-dev.txt`, `requirements/base.txt` and `-r` includes were
//! not, and nothing said so. PyPI recall measured 54% on 19 repositories.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use super::{DepScope, LockFormat, LockedPackage, LockfileOutcome, LockfileRead};

fn empty(format: LockFormat) -> LockfileRead {
    LockfileRead {
        path: PathBuf::new(),
        format,
        packages: Vec::new(),
        edges: Vec::new(),
        non_registry_entries: 0,
    }
}

/// `key = "value"` at the start of a line (no indent), unquoted.
fn toml_string(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
    let value = rest.trim();
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let inner = &value[1..];
    Some(inner[..inner.find(quote)?].to_string())
}

/// One `[[package]]` table's fields that decide what was installed.
#[derive(Default)]
struct TomlPackage {
    name: Option<String>,
    version: Option<String>,
    category: Option<String>,
    /// The project itself or local code (uv `editable`/`virtual`/`directory`/
    /// `path`, poetry `[package.source] type = "directory" | "file"`).
    local: bool,
}

fn finish(pkg: TomlPackage, read: &mut LockfileRead) {
    let (Some(name), Some(version)) = (pkg.name, pkg.version) else {
        return;
    };
    if pkg.local {
        read.non_registry_entries += 1;
        return;
    }
    let scope = match pkg.category.as_deref() {
        Some("dev") => DepScope::Dev,
        Some("main") => DepScope::Runtime,
        _ => DepScope::Unknown,
    };
    read.packages.push(
        LockedPackage::new(name, version)
            .scoped(scope)
            .primary(true),
    );
}

/// poetry.lock (every generation — pre-1.0 wrote keys alphabetically, so
/// `name` and `version` need not be adjacent), uv.lock and pdm.lock.
pub(super) fn read_toml_package_lock(content: &str, format: LockFormat) -> LockfileRead {
    let mut read = empty(format);
    let mut current: Option<TomlPackage> = None;
    let mut table = String::new();
    for raw in content.lines() {
        let line = raw.trim_end();
        if line.starts_with('[') {
            table = line.trim().to_string();
            if table == "[[package]]" {
                if let Some(done) = current.take() {
                    finish(done, &mut read);
                }
                current = Some(TomlPackage::default());
            }
            continue;
        }
        let Some(pkg) = current.as_mut() else {
            continue;
        };
        if table == "[[package]]" {
            if let Some(v) = toml_string(line, "name") {
                pkg.name = Some(v);
            } else if let Some(v) = toml_string(line, "version") {
                pkg.version = Some(v);
            } else if let Some(v) = toml_string(line, "category") {
                pkg.category = Some(v);
            } else if let Some(source) = line.strip_prefix("source") {
                // uv: `source = { editable = "." }`, `{ virtual = "." }`, ...
                let s = source.replace(' ', "");
                pkg.local |= ["{editable=", "{virtual=", "{directory=", "{path="]
                    .iter()
                    .any(|k| s.starts_with(&format!("={k}")));
            }
        } else if table == "[package.source]" {
            if let Some(kind) = toml_string(line, "type") {
                pkg.local |= kind == "directory" || kind == "file";
            }
        }
    }
    if let Some(done) = current.take() {
        finish(done, &mut read);
    }
    read
}

/// Pipfile.lock: `default` (runtime) and `develop` (dev) sections, versions
/// written `==1.2.3`. Git / path / editable entries carry no version.
pub(super) fn read_pipfile_lock(content: &str) -> Result<LockfileRead, String> {
    let lock: Value = serde_json::from_str(content).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut read = empty(LockFormat::Pipfile);
    let mut sections = 0;
    for (section, scope) in [("default", DepScope::Runtime), ("develop", DepScope::Dev)] {
        let Some(entries) = lock.get(section).and_then(Value::as_object) else {
            continue;
        };
        sections += 1;
        for (name, entry) in entries {
            let version = entry
                .get("version")
                .and_then(Value::as_str)
                .map(|v| v.trim_start_matches('=').trim());
            match version {
                Some(v) if !v.is_empty() => read.packages.push(
                    LockedPackage::new(name.as_str(), v)
                        .scoped(scope)
                        .primary(true),
                ),
                _ => read.non_registry_entries += 1,
            }
        }
    }
    if sections == 0 {
        return Err("no `default` or `develop` section".to_string());
    }
    Ok(read)
}

/// The requirements files a directory holds: any `*requirements*.txt` in it,
/// and every `.txt` in a `requirements/` subdirectory (`requirements/base.txt`).
/// A directory itself named `requirements` yields nothing — its parent owns it.
pub(crate) fn requirements_files(dir: &Path) -> Vec<PathBuf> {
    let is_requirements_dir = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("requirements"))
    };
    if is_requirements_dir(dir) {
        return Vec::new();
    }
    let txt_files = |d: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(d)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file())
                    .filter(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut out: Vec<PathBuf> = txt_files(dir)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.to_ascii_lowercase().contains("requirements"))
        })
        .collect();
    let sub = dir.join("requirements");
    if sub.is_dir() {
        out.extend(txt_files(&sub));
    }
    out.sort();
    out
}

/// Read a directory's requirements files, one outcome each.
pub(super) fn read_requirements_set(files: &[PathBuf]) -> Vec<LockfileOutcome> {
    files
        .iter()
        .map(|path| match std::fs::read_to_string(path) {
            Ok(content) => {
                let mut read = read_requirements(path, &content);
                read.path = path.clone();
                LockfileOutcome::Read(read)
            }
            Err(e) => LockfileOutcome::Failed {
                path: path.clone(),
                format: Some(LockFormat::Requirements),
                reason: format!("unreadable: {e}"),
            },
        })
        .collect()
}

/// Exact pins from one requirements file, following `-r` / `--requirement`
/// includes relative to it (cycle-guarded, depth-bounded).
pub(super) fn read_requirements(path: &Path, content: &str) -> LockfileRead {
    let mut read = empty(LockFormat::Requirements);
    let mut visited = HashSet::new();
    visited.insert(path.to_path_buf());
    collect_pins(path, content, 0, &mut visited, &mut read);
    read
}

const MAX_INCLUDE_DEPTH: usize = 5;

fn collect_pins(
    path: &Path,
    content: &str,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    read: &mut LockfileRead,
) {
    for line in logical_lines(content) {
        if let Some(include) = include_target(&line) {
            let target = path.parent().unwrap_or(Path::new(".")).join(include);
            if depth < MAX_INCLUDE_DEPTH && visited.insert(target.clone()) {
                if let Ok(nested) = std::fs::read_to_string(&target) {
                    collect_pins(&target, &nested, depth + 1, visited, read);
                }
            }
            continue;
        }
        if let Some((name, version)) = exact_pin(&line) {
            read.packages
                .push(LockedPackage::new(name, version).primary(true));
        }
    }
}

/// Physical lines joined across `\` continuations, comments removed. A `#`
/// starts a comment at line start or after whitespace (URLs may hold `#`).
fn logical_lines(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = String::new();
    for raw in content.lines() {
        let line = raw.trim_end();
        if let Some(head) = line.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
            continue;
        }
        pending.push_str(line);
        let mut text = std::mem::take(&mut pending);
        if let Some(hash) = comment_start(&text) {
            text.truncate(hash);
        }
        let text = text.trim().to_string();
        if !text.is_empty() {
            out.push(text);
        }
    }
    out
}

fn comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    (0..bytes.len()).find(|&i| bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()))
}

fn include_target(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("--requirement")
        .or_else(|| line.strip_prefix("-r"))?;
    let rest = rest.trim_start_matches('=').trim();
    (!rest.is_empty()).then_some(rest)
}

fn pin_regex() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([A-Za-z0-9][A-Za-z0-9._-]*)\s*(?:\[[^\]]*\])?\s*===?\s*([^\s,;]+)\s*$").ok()
    })
    .as_ref()
}

/// `name==1.2`, `name===1.2`, `name[extra]==1.2`, with environment markers
/// and per-requirement options (`--hash=…`) removed. Ranges, wildcards,
/// options, URLs and editables name no installed version.
fn exact_pin(line: &str) -> Option<(String, String)> {
    if line.starts_with('-') {
        return None;
    }
    let spec = line.split(';').next().unwrap_or(line);
    let spec: Vec<&str> = spec
        .split_whitespace()
        .filter(|token| !token.starts_with("--"))
        .collect();
    let spec = spec.join(" ");
    let caps = pin_regex()?.captures(spec.trim())?;
    let version = caps.get(2)?.as_str();
    if version.contains('*') {
        return None;
    }
    Some((caps.get(1)?.as_str().to_string(), version.to_string()))
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Manifests with no lockfile: what a project declares when nothing pins it.
//!
//! A directory holding only `package.json`, `Cargo.toml` or `pyproject.toml`
//! emitted nothing from [`super::read_dir`], so its dependencies vanished
//! uncounted while the advisory check read as complete (fresh-profile audit
//! 2026-11-11). Declared ranges are not installed versions and are never
//! matched against advisories; they are COUNTED here so the app can say what
//! it did not check — the same policy as the MCP server's
//! `coverage.unresolved_manifests` (4da-mcp-server#59: label, never suppress).
//!
//! A manifest counts as locked when a lockfile of its ecosystem sits beside it
//! or in a directory above it, up to and including the repository root (the
//! first directory holding `.git`): a Cargo or npm workspace member resolves
//! through the workspace root's lockfile.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use super::{FileSource, LockfileOutcome};

/// Manifests reported when unlocked, with the ACE language they declare for.
const MANIFESTS: &[(&str, &str)] = &[
    ("package.json", "javascript"),
    ("Cargo.toml", "rust"),
    ("pyproject.toml", "python"),
    ("Pipfile", "python"),
    ("Gemfile", "ruby"),
    ("composer.json", "php"),
];

/// Lockfiles that pin an ecosystem's installs. `bun.lockb` is reported as
/// unsupported on its own; it still means the manifest is locked.
fn lockfiles_for(ecosystem: &str) -> &'static [&'static str] {
    match ecosystem {
        "javascript" => &[
            "package-lock.json",
            "npm-shrinkwrap.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lock",
            "bun.lockb",
        ],
        "rust" => &["Cargo.lock"],
        "python" => &["poetry.lock", "uv.lock", "pdm.lock", "Pipfile.lock"],
        "ruby" => &["Gemfile.lock"],
        "php" => &["composer.lock"],
        _ => &[],
    }
}

/// Bound on the upward search for a lockfile (a path is never this deep).
const MAX_ANCESTORS: usize = 64;

/// The first manifest [`unlocked_manifests`] could report, for the walk's
/// probe. `None` when `dir` holds none.
pub(super) fn first_manifest(fs: &dyn FileSource, dir: &str) -> Option<String> {
    MANIFESTS
        .iter()
        .map(|(name, _)| fs.join(dir, name))
        .find(|p| fs.is_file(p))
}

/// One [`LockfileOutcome::ManifestOnly`] per manifest in `dir` that no
/// lockfile pins and that declares at least one registry dependency, or a
/// [`LockfileOutcome::Failed`] when it cannot be read. `pins_python`: the
/// directory's requirements files pinned something, which locks Python.
pub(super) fn unlocked_manifests(
    fs: &dyn FileSource,
    dir: &str,
    pins_python: bool,
) -> Vec<LockfileOutcome> {
    let mut out = Vec::new();
    for (name, ecosystem) in MANIFESTS {
        let path = fs.join(dir, name);
        if !fs.is_file(&path) {
            continue;
        }
        if (*ecosystem == "python" && pins_python) || locked(fs, dir, ecosystem) {
            continue;
        }
        let text = match fs.read(&path) {
            Ok(text) => text,
            Err(e) => {
                out.push(LockfileOutcome::Failed {
                    path,
                    format: None,
                    reason: format!("manifest unreadable: {e}"),
                });
                continue;
            }
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        match declared_dependencies(name, text) {
            Ok(0) => {}
            Ok(dependencies) => out.push(LockfileOutcome::ManifestOnly {
                path,
                ecosystem,
                dependencies,
            }),
            Err(reason) => out.push(LockfileOutcome::Failed {
                path,
                format: None,
                reason,
            }),
        }
    }
    out
}

/// A lockfile of `ecosystem` in `dir` or above it, up to the repository root.
fn locked(fs: &dyn FileSource, dir: &str, ecosystem: &str) -> bool {
    let names = lockfiles_for(ecosystem);
    let mut current = dir.to_string();
    for _ in 0..MAX_ANCESTORS {
        if names.iter().any(|n| fs.is_file(&fs.join(&current, n))) {
            return true;
        }
        let repo_root = fs
            .list(&current)
            .is_some_and(|entries| entries.iter().any(|e| e.name == ".git"));
        if repo_root {
            return false;
        }
        match fs.parent(&current) {
            Some(parent) if parent != current && !parent.is_empty() => current = parent,
            _ => return false,
        }
    }
    false
}

/// The number of distinct registry dependencies a manifest declares.
pub(super) fn declared_dependencies(name: &str, text: &str) -> Result<usize, String> {
    let names = match name {
        "package.json" => package_json(text)?,
        "composer.json" => composer_json(text)?,
        "Cargo.toml" => cargo_toml(text),
        "pyproject.toml" => pyproject_toml(text),
        "Pipfile" => key_tables(text, |t| t == "packages" || t == "dev-packages"),
        "Gemfile" => gemfile(text),
        _ => BTreeSet::new(),
    };
    Ok(names.len())
}

fn json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| format!("manifest is not valid JSON: {e}"))
}

/// `dependencies`, `devDependencies`, `optionalDependencies` (peers are the
/// consumer's to provide). Workspace, file, link and portal specs are local.
fn package_json(text: &str) -> Result<BTreeSet<String>, String> {
    let manifest = json(text)?;
    let mut names = BTreeSet::new();
    for section in ["dependencies", "devDependencies", "optionalDependencies"] {
        let Some(map) = manifest.get(section).and_then(Value::as_object) else {
            continue;
        };
        for (name, spec) in map {
            let spec = spec.as_str().unwrap_or("");
            let local = ["workspace:", "file:", "link:", "portal:"]
                .iter()
                .any(|p| spec.starts_with(p));
            if !local {
                names.insert(name.to_lowercase());
            }
        }
    }
    Ok(names)
}

/// `require` and `require-dev`, minus platform requirements (`php`,
/// `ext-*`, `lib-*`): a Packagist package name always has a vendor prefix.
fn composer_json(text: &str) -> Result<BTreeSet<String>, String> {
    let manifest = json(text)?;
    let mut names = BTreeSet::new();
    for section in ["require", "require-dev"] {
        if let Some(map) = manifest.get(section).and_then(Value::as_object) {
            names.extend(
                map.keys()
                    .filter(|n| n.contains('/'))
                    .map(|n| n.to_lowercase()),
            );
        }
    }
    Ok(names)
}

/// A TOML table header (`[a.b]`, `[[a]]`) as its dotted name, quotes removed.
fn table_header(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with('[') {
        return None;
    }
    let inner = line.trim_start_matches('[');
    let end = inner.find(']')?;
    Some(inner[..end].replace(['"', '\''], "").replace(' ', ""))
}

/// `key = value` at the start of a line, the key unquoted.
fn key_value(line: &str) -> Option<(String, &str)> {
    let line = line.trim();
    if line.starts_with('#') || line.starts_with('[') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let key = key.trim().trim_matches(['"', '\'']);
    (!key.is_empty() && !key.contains(' ')).then(|| (key.to_lowercase(), value.trim()))
}

/// An inline value naming no registry release: a local path or a VCS / URL
/// source without a version.
fn non_registry(value: &str) -> bool {
    let v = value.replace(' ', "");
    let local = ["path=", "git=", "url=", "file=", "editable="]
        .iter()
        .any(|k| v.contains(k));
    local && !v.contains("version=")
}

/// Whether a Cargo table holds dependencies (`[dependencies]`,
/// `[target.'cfg(..)'.dev-dependencies]`, `[workspace.dependencies]` ...).
fn cargo_dependency_table(table: &str) -> bool {
    ["dependencies", "dev-dependencies", "build-dependencies"]
        .iter()
        .any(|kind| {
            table == *kind
                || table == format!("workspace.{kind}")
                || (table.starts_with("target.") && table.ends_with(&format!(".{kind}")))
        })
}

#[derive(Default)]
struct SubTable {
    name: String,
    local: bool,
    version: bool,
    inherited: bool,
}

/// Cargo.toml: inline entries of every dependency table, and sub-tables
/// (`[dependencies.serde]`). `workspace = true` inherits an entry the
/// workspace root declares, so it is counted there.
fn cargo_toml(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut table = String::new();
    // A `[dependencies.<name>]` sub-table and what its keys say.
    let mut sub: Option<SubTable> = None;
    let flush = |sub: &mut Option<SubTable>, names: &mut BTreeSet<String>| {
        if let Some(s) = sub.take() {
            if (!s.local || s.version) && !s.inherited {
                names.insert(s.name);
            }
        }
    };
    for line in text.lines() {
        if let Some(header) = table_header(line) {
            flush(&mut sub, &mut names);
            if let Some((parent, name)) = header.rsplit_once('.') {
                if cargo_dependency_table(parent) {
                    sub = Some(SubTable {
                        name: name.to_lowercase(),
                        ..SubTable::default()
                    });
                }
            }
            table = header;
            continue;
        }
        let Some((key, value)) = key_value(line) else {
            continue;
        };
        if let Some(s) = sub.as_mut() {
            match key.as_str() {
                "path" | "git" => s.local = true,
                "version" => s.version = true,
                "workspace" => s.inherited = value.trim() == "true",
                _ => {}
            }
            continue;
        }
        if cargo_dependency_table(&table)
            && !non_registry(value)
            && !value.replace(' ', "").contains("workspace=true")
        {
            names.insert(key);
        }
    }
    flush(&mut sub, &mut names);
    names
}

/// Keys of the tables `wanted` selects, minus `python` (a version constraint
/// on the interpreter) and local / VCS entries (Pipfile, Poetry).
fn key_tables(text: &str, wanted: impl Fn(&str) -> bool) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut table = String::new();
    for line in text.lines() {
        if let Some(header) = table_header(line) {
            table = header;
            continue;
        }
        if !wanted(&table) {
            continue;
        }
        if let Some((key, value)) = key_value(line) {
            if key != "python" && !non_registry(value) {
                names.insert(key);
            }
        }
    }
    names
}

fn requirement_name_regex() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*([A-Za-z0-9][A-Za-z0-9._-]*)").ok())
        .as_ref()
}

/// A PEP 508 requirement's distribution name, PEP 503-normalised; `None` for
/// a direct URL reference (`name @ https://…`), which is no index release.
fn requirement_name(spec: &str) -> Option<String> {
    if spec.contains(" @ ") || spec.contains("://") {
        return None;
    }
    let name = requirement_name_regex()?.captures(spec)?.get(1)?.as_str();
    Some(name.to_lowercase().replace(['_', '.'], "-"))
}

/// Whether a pyproject array value holds requirement strings: PEP 621
/// `dependencies` and `optional-dependencies`, PEP 735 `dependency-groups`,
/// uv's and PDM's dev groups.
fn pyproject_array(table: &str, key: &str) -> bool {
    (table == "project" && key == "dependencies")
        || table == "project.optional-dependencies"
        || table == "dependency-groups"
        || (table == "tool.uv" && key == "dev-dependencies")
        || table == "tool.pdm.dev-dependencies"
}

/// Whether a pyproject table lists dependencies as keys (Poetry).
fn poetry_table(table: &str) -> bool {
    table == "tool.poetry.dependencies"
        || table == "tool.poetry.dev-dependencies"
        || (table.starts_with("tool.poetry.group.") && table.ends_with(".dependencies"))
}

/// pyproject.toml: PEP 621 / 735 arrays, uv and PDM dev groups, Poetry tables.
fn pyproject_toml(text: &str) -> BTreeSet<String> {
    let mut names = key_tables(text, poetry_table);
    let mut table = String::new();
    // Inside a requirement array: the text gathered so far.
    let mut array: Option<String> = None;
    for line in text.lines() {
        if let Some(buffer) = array.as_mut() {
            buffer.push('\n');
            buffer.push_str(line);
        } else if let Some(header) = table_header(line) {
            table = header;
            continue;
        } else if let Some((key, value)) = key_value(line) {
            if pyproject_array(&table, &key) && value.starts_with('[') {
                array = Some(value.to_string());
            }
        }
        if let Some(buffer) = array.as_ref() {
            if let Some(items) = closed_array(buffer) {
                names.extend(items.iter().filter_map(|s| requirement_name(s)));
                array = None;
            }
        }
    }
    names
}

/// The quoted strings of a TOML array once its closing `]` (outside quotes
/// and comments) has been read; `None` while it is still open.
fn closed_array(text: &str) -> Option<Vec<String>> {
    let mut items = Vec::new();
    let mut depth = 0i32;
    // Inside an inline table (`{ include-group = "dev" }`): its strings are
    // keys and values, not requirements.
    let mut braces = 0i32;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    let mut comment = false;
    for c in text.chars() {
        if comment {
            comment = c != '\n';
            continue;
        }
        match quote {
            Some(q) if c == q => {
                let item = std::mem::take(&mut current);
                if braces == 0 {
                    items.push(item);
                }
                quote = None;
            }
            Some(_) => current.push(c),
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => comment = true,
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(items);
                    }
                }
                '{' => braces += 1,
                '}' => braces -= 1,
                _ => {}
            },
        }
    }
    None
}

fn gem_regex() -> Option<&'static Regex> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"^\s*gem\s+["']([^"']+)["']"#).ok())
        .as_ref()
}

/// Gemfile `gem "name"` lines; `path:`, `git:` and `github:` gems are local
/// code or a checkout, not RubyGems releases.
fn gemfile(text: &str) -> BTreeSet<String> {
    let Some(re) = gem_regex() else {
        return BTreeSet::new();
    };
    text.lines()
        .filter(|l| {
            !["path:", "git:", "github:", ":path", ":git"]
                .iter()
                .any(|k| l.contains(k))
        })
        .filter_map(|l| re.captures(l)?.get(1).map(|m| m.as_str().to_lowercase()))
        .collect()
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;

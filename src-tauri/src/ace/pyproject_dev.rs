// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The development-only dependencies a `pyproject.toml` declares.
//!
//! None were read before 2026-10-26 (6.2.5 parity audit, A-4): a dependency
//! declared only in PEP 735 `[dependency-groups]` — uv's `dev = [...]` —
//! reached the store as an undeclared, unscoped copy, so it was graded as a
//! transitive runtime package instead of a direct dev one. The MCP engine
//! read the same table as dev (`project-manifests.ts`); this is the app's
//! side of that rule.
//!
//! Development declarations read here:
//! - PEP 735 `[dependency-groups]`: every group, its `{include-group = "x"}`
//!   entries followed (cycle-guarded).
//! - uv's legacy `[tool.uv] dev-dependencies = [...]`.
//! - PDM's `[tool.pdm.dev-dependencies]` groups.
//! - Poetry's `[tool.poetry.dev-dependencies]` and every
//!   `[tool.poetry.group.<name>.dependencies]` table.
//!
//! Every group counts as development, whatever its name and whether uv's
//! `[tool.uv] default-groups` installs it by default: a dependency group is
//! never part of the package's own requirements (PEP 735: groups "are not
//! included in the package's metadata"), so it ships with no install of the
//! project. `default-groups` decides what `uv sync` puts in the developer's
//! environment, not what runs in production. A name the project ALSO
//! declares as a runtime requirement stays runtime; the caller removes those.

use std::collections::{BTreeMap, HashSet};

use super::scanner::{closes_toml_array, pep508_package_name, strip_toml_comment};

/// PEP 735 / PEP 503 name normalisation (group names compare this way too).
fn normalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut sep = false;
    for c in name.trim().chars() {
        if matches!(c, '-' | '_' | '.') {
            sep = true;
        } else {
            if sep && !out.is_empty() {
                out.push('-');
            }
            sep = false;
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// One array of a group table: its requirement strings and include-groups.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct GroupEntries {
    requirements: Vec<String>,
    includes: Vec<String>,
}

/// The elements of a TOML array body (`"ruff>=0.4", {include-group = "test"}`):
/// quoted strings at the top level are requirements; an inline table's
/// `include-group` value is an include.
fn array_entries(body: &str) -> GroupEntries {
    let mut out = GroupEntries::default();
    let mut quote: Option<char> = None;
    let mut current = String::new();
    let mut table: Option<String> = None;
    for c in body.chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                match table.as_mut() {
                    Some(t) => {
                        t.push(c);
                    }
                    None => out.requirements.push(std::mem::take(&mut current)),
                }
            }
            Some(_) => match table.as_mut() {
                Some(t) => t.push(c),
                None => current.push(c),
            },
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    if let Some(t) = table.as_mut() {
                        t.push(c);
                    }
                }
                '{' => table = Some(String::new()),
                '}' => {
                    if let Some(t) = table.take() {
                        if let Some(group) = include_group(&t) {
                            out.includes.push(group);
                        }
                    }
                }
                _ => {
                    if let Some(t) = table.as_mut() {
                        t.push(c);
                    }
                }
            },
        }
    }
    out
}

/// `include-group = "test"` inside an inline table's text.
fn include_group(table: &str) -> Option<String> {
    let rest = table.split_once("include-group")?.1.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let q = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let inner = &rest[1..];
    Some(inner[..inner.find(q)?].to_string())
}

/// A bare or quoted TOML key that can name a package (`requests`,
/// `"zope.interface"`); `None` for continuation lines of a multi-line value.
fn table_key(line: &str) -> Option<String> {
    let (key, _) = line.split_once('=')?;
    let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    valid.then(|| key.to_string())
}

/// What kind of table a header opens.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Section {
    Other,
    /// `[dependency-groups]`, `[tool.pdm.dev-dependencies]`: `group = [...]`.
    GroupArrays,
    /// `[tool.uv]`: only its `dev-dependencies = [...]` array.
    Uv,
    /// Poetry dev tables: `pkg = "<constraint>"` entries.
    PoetryDev,
}

fn section_of(header: &str) -> (Section, Option<String>) {
    let header = header.trim();
    match header {
        "dependency-groups" | "tool.pdm.dev-dependencies" => (Section::GroupArrays, None),
        "tool.uv" => (Section::Uv, None),
        "tool.poetry.dev-dependencies" => (Section::PoetryDev, None),
        _ => {
            // `[tool.poetry.group.<g>.dependencies]` is a dev table;
            // `[tool.poetry.group.<g>.dependencies.<pkg>]` and
            // `[tool.poetry.dev-dependencies.<pkg>]` declare `<pkg>` itself.
            if let Some(rest) = header.strip_prefix("tool.poetry.group.") {
                if let Some((_, tail)) = rest.split_once(".dependencies") {
                    return match tail.strip_prefix('.') {
                        Some(pkg) => (Section::Other, Some(pkg.trim_matches('"').to_string())),
                        None if tail.is_empty() => (Section::PoetryDev, None),
                        None => (Section::Other, None),
                    };
                }
            }
            if let Some(pkg) = header.strip_prefix("tool.poetry.dev-dependencies.") {
                return (Section::Other, Some(pkg.trim_matches('"').to_string()));
            }
            (Section::Other, None)
        }
    }
}

/// Everything the file declares for development, grouped: PEP 735 / PDM
/// groups under their normalised names (includes NOT yet followed), uv's
/// legacy list under `dev`, Poetry tables under `poetry`.
fn raw_groups(content: &str) -> BTreeMap<String, GroupEntries> {
    let mut groups: BTreeMap<String, GroupEntries> = BTreeMap::new();
    let mut section = Section::Other;
    // An array being read: (group, accumulated body).
    let mut array: Option<(String, String)> = None;
    for raw in content.lines() {
        let line = strip_toml_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some((group, body)) = array.as_mut() {
            body.push(' ');
            body.push_str(line);
            if closes_toml_array(line) {
                let entries = array_entries(body);
                let slot = groups.entry(group.clone()).or_default();
                slot.requirements.extend(entries.requirements);
                slot.includes.extend(entries.includes);
                array = None;
            }
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') && !line.starts_with("[[") {
            let (next, declared) = section_of(&line[1..line.len() - 1]);
            section = next;
            if let Some(pkg) = declared {
                groups
                    .entry("poetry".to_string())
                    .or_default()
                    .requirements
                    .push(pkg);
            }
            continue;
        }
        let group = match &section {
            Section::GroupArrays => table_key(line).map(|k| normalize(&k)),
            Section::Uv => table_key(line)
                .filter(|k| k == "dev-dependencies")
                .map(|_| "dev".to_string()),
            Section::PoetryDev => {
                if let Some(key) = table_key(line) {
                    groups
                        .entry("poetry".to_string())
                        .or_default()
                        .requirements
                        .push(key);
                }
                None
            }
            Section::Other => None,
        };
        let Some(group) = group else {
            continue;
        };
        let Some((_, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim_start();
        let Some(body) = value.strip_prefix('[') else {
            continue;
        };
        if closes_toml_array(body) {
            let entries = array_entries(body);
            let slot = groups.entry(group).or_default();
            slot.requirements.extend(entries.requirements);
            slot.includes.extend(entries.includes);
        } else {
            array = Some((group, body.to_string()));
        }
    }
    groups
}

/// Every group's requirement names with its `include-group` entries
/// followed, transitively and cycle-guarded (PEP 735 makes a cycle an error;
/// here it just stops). Names are as written, minus versions and markers.
pub(crate) fn dependency_groups(content: &str) -> BTreeMap<String, Vec<String>> {
    let raw = raw_groups(content);
    let mut out = BTreeMap::new();
    for group in raw.keys() {
        let mut names: Vec<String> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut stack = vec![group.clone()];
        while let Some(g) = stack.pop() {
            if !visited.insert(g.clone()) {
                continue;
            }
            let Some(entries) = raw.get(&g) else {
                continue;
            };
            for spec in &entries.requirements {
                if let Some(name) = pep508_package_name(spec) {
                    if !name.eq_ignore_ascii_case("python") && !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            stack.extend(entries.includes.iter().map(|i| normalize(i)));
        }
        out.insert(group.clone(), names);
    }
    out
}

/// Every package the file declares for development, in first-seen order.
pub(crate) fn parse_pyproject_dev_dependencies(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for names in dependency_groups(content).into_values() {
        for name in names {
            if !out.iter().any(|n| normalize(n) == normalize(&name)) {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "pyproject_dev_tests.rs"]
mod tests;

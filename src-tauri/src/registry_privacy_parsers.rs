// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The configuration-file readers behind `registry_privacy` (pure: text in,
//! names out), and Go's private-pattern matching.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{NPM_PUBLIC_HOSTS, PYPI_PUBLIC_HOSTS};

// ============================================================================
// Parsers (pure)
// ============================================================================

/// `KEY=value` lines (the `go env -w` file).
pub(super) fn parse_env_file(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// Whether `url` is on one of `hosts` (or a subdomain of one).
pub(super) fn url_on(url: &str, hosts: &[&str]) -> bool {
    let raw = url.trim().trim_matches(|c| c == '"' || c == '\'');
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let Ok(parsed) = url::Url::parse(&with_scheme) else {
        return false;
    };
    let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    hosts
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

/// An `.npmrc` whose default `registry=` is not the public one.
pub(crate) fn npmrc_default_registry_is_private(npmrc: &str) -> bool {
    npmrc.lines().map(str::trim).any(|line| {
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        key.trim() == "registry" && !value.trim().is_empty() && !url_on(value, NPM_PUBLIC_HOSTS)
    })
}

/// A list of index URLs (space or comma separated, uv's `name=url` too) that
/// names one other than pypi.org.
pub(super) fn names_private_index(value: &str) -> bool {
    value
        .split([' ', ','])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.split_once('=').map_or(v, |(_, url)| url))
        .any(|url| !url_on(url, PYPI_PUBLIC_HOSTS))
}

/// A pip (`index-url = ...`) or uv (`index-url`, `[[index]] url = ...`)
/// configuration that names an index other than pypi.org.
pub(super) fn python_config_private(text: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        let key = key.trim().to_ascii_lowercase().replace('_', "-");
        matches!(
            key.as_str(),
            "index-url" | "extra-index-url" | "url" | "default-index"
        ) && names_private_index(value.trim().trim_matches(|c| c == '"' || c == '\''))
    })
}

/// `[[package]]` entries of a `Cargo.lock` whose `source` is not crates.io:
/// no source (the project's own crates, path dependencies), git, or another
/// registry.
pub(crate) fn cargo_lock_private(lock: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut source: Option<String> = None;
    let mut in_package = false;
    let flush = |name: &mut Option<String>, source: &mut Option<String>, out: &mut Vec<String>| {
        if let Some(n) = name.take() {
            let public = source.as_deref().is_some_and(|s| {
                s == "registry+https://github.com/rust-lang/crates.io-index"
                    || s == "sparse+https://index.crates.io/"
            });
            if !public {
                out.push(n);
            }
        }
        *source = None;
    };
    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_package {
                flush(&mut name, &mut source, &mut out);
            }
            in_package = trimmed == "[[package]]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(v) = toml_string_value(trimmed, "name") {
            name = Some(v);
        } else if let Some(v) = toml_string_value(trimmed, "source") {
            source = Some(v);
        }
    }
    if in_package {
        flush(&mut name, &mut source, &mut out);
    }
    out
}

/// `key = "value"` on one line.
pub(super) fn toml_string_value(line: &str, key: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
}

/// Crates a `Cargo.toml` takes from `path`, `git` or a non-default
/// `registry`, plus `[package] name`. Inline tables only (the form every
/// generator writes); a `[dependencies.foo]` table is read too.
pub(crate) fn cargo_toml_private(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut table_dep: Option<(String, Option<String>, bool)> = None;
    let close = |dep: &mut Option<(String, Option<String>, bool)>, out: &mut Vec<String>| {
        if let Some((key, package, private)) = dep.take() {
            if private {
                out.push(package.unwrap_or(key));
            }
        }
    };
    for line in manifest.lines() {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') {
            close(&mut table_dep, &mut out);
            section = trimmed.trim_matches(|c| c == '[' || c == ']').to_string();
            if let Some(dep) = dependency_table_name(&section) {
                table_dep = Some((dep, None, false));
            }
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim();
        if let Some(dep) = table_dep.as_mut() {
            match key {
                "path" | "git" => dep.2 = true,
                "registry" => dep.2 = true,
                "package" => dep.1 = Some(value.trim_matches('"').to_string()),
                _ => {}
            }
            continue;
        }
        if section == "package" && key == "name" {
            out.push(value.trim_matches('"').to_string());
            continue;
        }
        if is_dependency_section(&section) && value.starts_with('{') {
            let private = ["path", "git", "registry"]
                .iter()
                .any(|k| inline_has_key(value, k));
            if private {
                out.push(inline_string(value, "package").unwrap_or_else(|| key.to_string()));
            }
        }
    }
    close(&mut table_dep, &mut out);
    out
}

pub(super) fn is_dependency_section(section: &str) -> bool {
    section == "dependencies"
        || section == "dev-dependencies"
        || section == "build-dependencies"
        || section == "workspace.dependencies"
        || (section.starts_with("target.")
            && (section.ends_with(".dependencies")
                || section.ends_with(".dev-dependencies")
                || section.ends_with(".build-dependencies")))
}

/// `dependencies.foo` (and the dev/build/workspace/target forms) → `foo`.
pub(super) fn dependency_table_name(section: &str) -> Option<String> {
    for prefix in [
        "dependencies.",
        "dev-dependencies.",
        "build-dependencies.",
        "workspace.dependencies.",
    ] {
        if let Some(name) = section.strip_prefix(prefix) {
            return Some(name.trim_matches('"').to_string());
        }
    }
    None
}

pub(super) fn inline_has_key(table: &str, key: &str) -> bool {
    table
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .any(|(k, _)| k.trim() == key)
}

pub(super) fn inline_string(table: &str, key: &str) -> Option<String> {
    table
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
}

/// From a `go.mod`: the module's own path, and every module a `replace`
/// directive points at a local directory.
pub(crate) fn go_mod_private(gomod: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_replace = false;
    for line in gomod.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(module) = line.strip_prefix("module ") {
            out.push(module.trim().trim_matches('"').to_string());
            continue;
        }
        if line == "replace (" {
            in_replace = true;
            continue;
        }
        if in_replace && line == ")" {
            in_replace = false;
            continue;
        }
        let directive = if in_replace {
            Some(line)
        } else {
            line.strip_prefix("replace ")
        };
        if let Some((from, to)) = directive.and_then(|d| d.split_once("=>")) {
            let target = to.split_whitespace().next().unwrap_or("");
            if is_local_path(target) {
                if let Some(module) = from.split_whitespace().next() {
                    out.push(module.to_string());
                }
            }
        }
    }
    out
}

pub(super) fn is_local_path(target: &str) -> bool {
    target.starts_with("./")
        || target.starts_with("../")
        || target.starts_with(".\\")
        || target.starts_with("..\\")
        || target.starts_with('/')
        || target.as_bytes().get(1) == Some(&b':')
}

/// What a requirements file says about where its packages come from.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct PythonFacts {
    /// An index other than pypi.org is named (`-i`, `--index-url`,
    /// `--extra-index-url`, `[[tool.uv.index]]`, `[[tool.poetry.source]]`).
    pub other_index: bool,
    /// Packages that come from a URL or path, and the project's own name.
    pub names: Vec<String>,
}

pub(crate) fn requirements_private(text: &str) -> PythonFacts {
    let mut out = PythonFacts::default();
    for line in text.lines() {
        let line = line.split(" #").next().unwrap_or("").trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        for flag in [
            "--index-url",
            "--extra-index-url",
            "-i",
            "--find-links",
            "-f",
        ] {
            if let Some(rest) = line.strip_prefix(flag) {
                let url = rest.trim_start_matches('=').trim();
                if (rest.starts_with(' ') || rest.starts_with('=')) && !url.is_empty() {
                    out.other_index |= !url_on(url, PYPI_PUBLIC_HOSTS);
                }
            }
        }
        if let Some((name, target)) = line.split_once(" @ ") {
            let target = target.trim();
            if !target.is_empty() {
                out.names.push(requirement_name(name));
            }
        }
    }
    out
}

/// The distribution name of a requirement (`foo[extra]>=1` → `foo`).
pub(super) fn requirement_name(spec: &str) -> String {
    spec.trim()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .next()
        .unwrap_or("")
        .to_string()
}

/// `pyproject.toml`:
/// - the project's own name (`[project]`, `[tool.poetry]`);
/// - whether it names another index (`[[tool.uv.index]]`, `[tool.uv]
///   index-url`, `[[tool.poetry.source]]`, `[[tool.pdm.source]]`);
/// - dependencies that come from somewhere other than an index: PEP 621
///   `"x @ git+..."` / `"x @ file:..."`, Poetry `x = { git | path | url |
///   source = ... }` and uv `[tool.uv.sources] x = { git | path | url |
///   index | workspace = ... }`. These work without a lockfile.
pub(crate) fn pyproject_private(text: &str) -> PythonFacts {
    let mut out = PythonFacts::default();
    let mut section = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with("[\"") && !trimmed.starts_with("['") {
            section = trimmed
                .split('#')
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .to_string();
            if matches!(
                section.as_str(),
                "tool.uv.index" | "tool.poetry.source" | "tool.pdm.source"
            ) {
                out.other_index = true;
            }
            continue;
        }
        if section == "project" || section.starts_with("project.optional-dependencies") {
            out.names.extend(direct_reference_names(trimmed));
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim();
        if (section == "project" || section == "tool.poetry") && key == "name" {
            out.names
                .push(value.trim_matches('"').trim_matches('\'').to_string());
        }
        if section == "tool.uv" && matches!(key, "index-url" | "extra-index-url") {
            out.other_index |= names_private_index(value.trim_matches('"').trim_matches('\''));
        }
        let poetry_deps = section == "tool.poetry.dependencies"
            || section == "tool.poetry.dev-dependencies"
            || (section.starts_with("tool.poetry.group.") && section.ends_with(".dependencies"));
        if poetry_deps
            && value.starts_with('{')
            && ["git", "path", "url", "source"]
                .iter()
                .any(|k| inline_has_key(value, k))
        {
            out.names.push(key.to_string());
        }
        if section == "tool.uv.sources"
            && value.starts_with('{')
            && ["git", "path", "url", "index", "workspace"]
                .iter()
                .any(|k| inline_has_key(value, k))
        {
            out.names.push(key.to_string());
        }
    }
    out
}

/// The names of the PEP 508 direct references (`"name @ url"`) quoted on
/// one line of a dependency array.
pub(super) fn direct_reference_names(line: &str) -> Vec<String> {
    line.split('"')
        .skip(1)
        .step_by(2)
        .chain(line.split('\'').skip(1).step_by(2))
        .filter_map(|spec| {
            spec.split_once(" @ ")
                .map(|(name, _)| requirement_name(name))
        })
        .filter(|n| !n.is_empty())
        .collect()
}

/// `Pipfile`: a `[[source]]` other than pypi.org, and packages taken from a
/// VCS, a path, a file or a named non-default index.
pub(crate) fn pipfile_private(text: &str) -> PythonFacts {
    let mut out = PythonFacts::default();
    let mut section = String::new();
    for line in text.lines() {
        let trimmed = line.split(" #").next().unwrap_or("").trim();
        if trimmed.starts_with('[') {
            section = trimmed.trim_matches(|c| c == '[' || c == ']').to_string();
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim();
        if section == "source" && key == "url" {
            out.other_index |= !url_on(value.trim_matches('"'), PYPI_PUBLIC_HOSTS);
        }
        if (section == "packages" || section == "dev-packages")
            && value.starts_with('{')
            && ["git", "path", "file", "hg", "svn", "bzr", "index"]
                .iter()
                .any(|k| inline_has_key(value, k))
        {
            out.names.push(key.to_string());
        }
    }
    out
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches('"').trim_matches('\'').to_string()
}

/// What a Yarn configuration file (`.yarnrc.yml` for Yarn 2+, `.yarnrc` for
/// Yarn 1) says: whether its default registry is not the public one, and the
/// scopes it routes to a non-public registry.
pub(crate) fn yarnrc_private(text: &str) -> (bool, HashSet<String>) {
    let mut default_private = false;
    let mut scopes = HashSet::new();
    let mut in_scopes = false;
    let mut current_scope: Option<String> = None;
    for raw in text.lines() {
        let line = raw.split(" #").next().unwrap_or("");
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // Yarn 2+: `key: value` YAML.
        let yaml = trimmed
            .split_once(':')
            .filter(|(k, _)| !trimmed.starts_with('"') && !k.trim().contains(char::is_whitespace));
        if let Some((key, value)) = yaml {
            let key = key.trim().trim_matches('"').trim_matches('\'');
            if indent == 0 {
                in_scopes = key == "npmScopes";
                current_scope = None;
                if key == "npmRegistryServer" {
                    let url = unquote(value);
                    default_private |= !url.is_empty() && !url_on(&url, NPM_PUBLIC_HOSTS);
                }
                continue;
            }
            if in_scopes && value.trim().is_empty() && !key.starts_with("npm") {
                current_scope = Some(format!(
                    "@{}",
                    key.trim_start_matches('@').to_ascii_lowercase()
                ));
                continue;
            }
            if in_scopes && key == "npmRegistryServer" {
                if let Some(scope) = &current_scope {
                    let url = unquote(value);
                    if !url.is_empty() && !url_on(&url, NPM_PUBLIC_HOSTS) {
                        scopes.insert(scope.clone());
                    }
                }
            }
            continue;
        }
        // Yarn 1: `key "value"` or `"@scope:registry" "value"`.
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or("").trim_matches('"');
        let value = unquote(parts.next().unwrap_or(""));
        if key == "registry" {
            default_private |= !value.is_empty() && !url_on(&value, NPM_PUBLIC_HOSTS);
        } else if let Some(scope) = key.strip_suffix(":registry") {
            if scope.starts_with('@') && !value.is_empty() && !url_on(&value, NPM_PUBLIC_HOSTS) {
                scopes.insert(scope.to_ascii_lowercase());
            }
        }
    }
    (default_private, scopes)
}

/// A `GOPROXY` value that does not include the public proxy (a corporate
/// proxy, `direct`, `off`): the user keeps module paths away from
/// `proxy.golang.org`, so 4DA sends none.
pub(crate) fn goproxy_is_private(value: &str) -> bool {
    let entries: Vec<&str> = value
        .split([',', '|'])
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    !entries.is_empty() && !entries.iter().any(|e| url_on(e, &["proxy.golang.org"]))
}

/// `uv.lock` / `poetry.lock` packages taken from anywhere but pypi.org.
pub(crate) fn python_lock_private(lock: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut private = false;
    let mut flush = |name: &mut Option<String>, private: &mut bool| {
        if let Some(n) = name.take() {
            if *private {
                out.push(n);
            }
        }
        *private = false;
    };
    let mut in_source = false;
    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            flush(&mut name, &mut private);
            in_source = false;
            continue;
        }
        if trimmed.starts_with('[') {
            // poetry: `[package.source]` belongs to the current package.
            in_source = trimmed == "[package.source]";
            continue;
        }
        if name.is_none() {
            if let Some(v) = toml_string_value(trimmed, "name") {
                name = Some(v);
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("source") {
            // uv: `source = { registry = "https://pypi.org/simple" }`.
            let rest = rest.trim_start();
            if let Some(table) = rest.strip_prefix('=') {
                let table = table.trim();
                private |= match inline_string(table, "registry") {
                    Some(url) => !url_on(&url, PYPI_PUBLIC_HOSTS),
                    None => true,
                };
            }
        }
        if in_source {
            // poetry: a `[package.source]` exists only for a non-PyPI source.
            if let Some(kind) = toml_string_value(trimmed, "type") {
                private |= kind != "pypi";
            }
        }
    }
    flush(&mut name, &mut private);
    out
}

/// The requirements files at a project's root.
pub(super) fn requirement_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let is_txt = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt"));
                    is_txt
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.contains("requirements"))
                })
                .collect()
        })
        .unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(root.join("requirements")) {
        files.extend(
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "txt")),
        );
    }
    files
}

// ============================================================================
// Go's private-pattern matching (`golang.org/x/mod/module.MatchPrefixPatterns`)
// ============================================================================

/// Whether `module` matches any of `patterns`. A pattern with N slashes is
/// matched (`path.Match`) against the first N+1 elements of the path, so
/// `github.com/corp` covers every module under it and `*.corp.example` every
/// module on those hosts.
pub(crate) fn go_matches_any(patterns: &[String], module: &str) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim().trim_end_matches('/');
        if pattern.is_empty() {
            return false;
        }
        let mut slashes = pattern.matches('/').count();
        let mut prefix = module;
        for (i, b) in module.bytes().enumerate() {
            if b == b'/' {
                if slashes == 0 {
                    prefix = &module[..i];
                    break;
                }
                slashes -= 1;
            }
        }
        if slashes > 0 {
            return false;
        }
        glob_match(pattern.as_bytes(), prefix.as_bytes())
    })
}

/// `path.Match`: `*` any run of non-`/` bytes, `?` one non-`/` byte, `[...]`
/// a class (`a-z` ranges, leading `^` or `!` negates), `\` escapes.
pub(super) fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => {
            let mut i = 0;
            loop {
                if glob_match(rest, &name[i..]) {
                    return true;
                }
                if i == name.len() || name[i] == b'/' {
                    return false;
                }
                i += 1;
            }
        }
        Some((b'?', rest)) => match name.split_first() {
            Some((c, tail)) if *c != b'/' => glob_match(rest, tail),
            _ => false,
        },
        Some((b'[', rest)) => {
            let Some((c, tail)) = name.split_first() else {
                return false;
            };
            let Some(end) = rest.iter().position(|b| *b == b']') else {
                return false;
            };
            let (class, after) = (&rest[..end], &rest[end + 1..]);
            let (negate, class) = match class.split_first() {
                Some((b'^' | b'!', more)) => (true, more),
                _ => (false, class),
            };
            let mut hit = false;
            let mut i = 0;
            while i < class.len() {
                if i + 2 < class.len() && class[i + 1] == b'-' {
                    hit |= class[i] <= *c && *c <= class[i + 2];
                    i += 3;
                } else {
                    hit |= class[i] == *c;
                    i += 1;
                }
            }
            hit != negate && *c != b'/' && glob_match(after, tail)
        }
        Some((b'\\', rest)) if !rest.is_empty() => match name.split_first() {
            Some((c, tail)) if *c == rest[0] => glob_match(&rest[1..], tail),
            _ => false,
        },
        Some((p, rest)) => match name.split_first() {
            Some((c, tail)) if c == p => glob_match(rest, tail),
            _ => false,
        },
    }
}

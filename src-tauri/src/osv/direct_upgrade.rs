// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! How a DIRECT dependency moves to its fix target, worded for the tool and
//! the manifest that declares it (audit E8: a direct row said only
//! "4.2 -> 5.2.17").
//!
//! A command is named only where it was checked against the tool to move the
//! dependency and keep it as declared (2026-10-15): npm 10.9.2, pnpm 10.34.6,
//! yarn 1.22.22, yarn 4.9.1, cargo 1.95 and go 1.24. Several naive commands
//! would have rewritten the declaration:
//! - `npm install x@1.2.8` writes `^1.2.8` over an exact pin and over `~1.2.5`;
//!   `--save-exact` or the declared operator keeps it;
//! - `yarn upgrade x@1.2.8` (yarn 1) and `yarn up x@1.2.8` (berry) write an
//!   exact pin over `^`/`~`; the operator in the spec keeps it;
//! - `cargo add` on a `workspace = true` dependency replaces the inheritance
//!   with a local pin, so the workspace root must move instead;
//! - `cargo add` on a dev or build dependency writes a second
//!   `[dependencies]` entry unless given `--dev` / `--build`.
//!
//! Anything this cannot read with certainty (two declarations, a git or path
//! spec, a `>=`/`||` range, an unknown tool) names nothing: the plan keeps
//! its plain "bump it where it is declared".

use std::path::Path;

use super::fix_path::Manager;

/// What to tell the user, per declaring project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectFix {
    /// One command, run in the project's directory.
    Command(String),
    /// A manifest edit no single verified command makes.
    Instruction(String),
}

impl DirectFix {
    /// The text shown in the plan: a command in backticks, an instruction as is.
    pub fn render(&self) -> String {
        match self {
            Self::Command(c) => format!("`{c}`"),
            Self::Instruction(i) => i.clone(),
        }
    }
}

/// The fix for `package` declared in the project at `project_dir`, read from
/// its own manifest. `None` when it cannot be named with certainty.
pub fn direct_fix(
    project_dir: &Path,
    ecosystem_norm: &str,
    package: &str,
    target: &str,
) -> Option<DirectFix> {
    let read = |file: &str| std::fs::read_to_string(project_dir.join(file)).ok();
    match ecosystem_norm {
        "npm" => {
            let manager = Manager::detect(project_dir, "npm")?;
            let declared = npm_declared(&read("package.json")?, package)?;
            npm_direct_command(manager, &declared, package, target).map(DirectFix::Command)
        }
        "crates.io" => {
            let decls = cargo_declarations(&read("Cargo.toml")?, package);
            match decls.as_slice() {
                [only] => cargo_direct(only, package, target),
                _ => None,
            }
        }
        "Go" => go_direct_command(&read("go.mod")?, package, target).map(DirectFix::Command),
        "PyPI" => [
            "requirements.txt",
            "requirements-dev.txt",
            "requirements/base.txt",
        ]
        .iter()
        .find_map(|f| pypi_direct_instruction(&read(f)?, f, package, target))
        .map(DirectFix::Instruction),
        _ => None,
    }
}

/// The spec `package` is declared with in a `package.json`, when exactly one
/// section (`dependencies`, `devDependencies`, `optionalDependencies`) holds it.
pub(crate) fn npm_declared(package_json: &str, package: &str) -> Option<String> {
    let pkg: serde_json::Value = serde_json::from_str(package_json).ok()?;
    let found: Vec<&str> = ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .filter_map(|s| pkg.get(*s)?.get(package)?.as_str())
        .collect();
    match found.as_slice() {
        [only] => Some((*only).to_string()),
        _ => None,
    }
}

/// The operator a declared npm spec carries: `""` for an exact pin, `^` or
/// `~`. `None` for anything else (ranges, tags, git, `workspace:`, aliases).
fn npm_operator(declared: &str) -> Option<&'static str> {
    let d = declared.trim();
    let digits = |s: &str| {
        let parts: Vec<&str> = s.split('.').collect();
        (1..=3).contains(&parts.len())
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    };
    if let Some(rest) = d.strip_prefix('^') {
        return digits(rest).then_some("^");
    }
    if let Some(rest) = d.strip_prefix('~') {
        return digits(rest).then_some("~");
    }
    let exact = d.strip_prefix('=').unwrap_or(d).trim_start_matches('v');
    semver::Version::parse(exact).is_ok().then_some("")
}

/// The command that moves a declared npm-family dependency to `target`,
/// keeping its declared operator. Specs with an operator are quoted: `^` is
/// cmd.exe's escape character.
pub fn npm_direct_command(
    manager: Manager,
    declared: &str,
    package: &str,
    target: &str,
) -> Option<String> {
    let op = npm_operator(declared)?;
    let spec = if op.is_empty() {
        format!("{package}@{target}")
    } else {
        format!("\"{package}@{op}{target}\"")
    };
    Some(match manager {
        Manager::Npm if op.is_empty() => format!("npm install {spec} --save-exact"),
        Manager::Npm => format!("npm install {spec}"),
        Manager::Pnpm | Manager::PnpmWorkspace => format!("pnpm up {spec}"),
        Manager::YarnClassic => format!("yarn upgrade {spec}"),
        Manager::YarnBerry => format!("yarn up {spec}"),
        Manager::Cargo => return None,
    })
}

/// Which Cargo table declares a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoKind {
    Normal,
    Dev,
    Build,
}

/// One declaration of a crate in a `Cargo.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoDecl {
    pub kind: CargoKind,
    /// `cfg(windows)` or a target triple, for `[target.<t>.*dependencies]`.
    pub target: Option<String>,
    /// The key it is declared under (differs from the crate for a rename).
    pub key: String,
    /// The version requirement as written, when one is.
    pub req: Option<String>,
    /// `workspace = true`: the version lives in the workspace root.
    pub workspace: bool,
}

/// Every declaration of `package` in a `Cargo.toml` (by crate name, so a
/// renamed dependency is found through its `package = ...`). Line-based, like
/// `fix_path::cargo_requirements`: a manifest this cannot read yields nothing.
pub fn cargo_declarations(manifest: &str, package: &str) -> Vec<CargoDecl> {
    let want = crate_key(package);
    let mut out: Vec<CargoDecl> = Vec::new();
    // (kind, target) of the current `[..dependencies]` table, or the same
    // plus the key of a `[..dependencies.<key>]` table.
    let mut table: Option<(CargoKind, Option<String>, Option<String>)> = None;
    let mut pending: Option<(CargoDecl, Option<String>)> = None;
    let flush = |p: &mut Option<(CargoDecl, Option<String>)>, out: &mut Vec<CargoDecl>| {
        if let Some((decl, crate_name)) = p.take() {
            if crate_key(crate_name.as_deref().unwrap_or(&decl.key)) == want {
                out.push(decl);
            }
        }
    };
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            flush(&mut pending, &mut out);
            table = cargo_table(line);
            if let Some((kind, target, Some(key))) = &table {
                pending = Some((
                    CargoDecl {
                        kind: *kind,
                        target: target.clone(),
                        key: key.clone(),
                        req: None,
                        workspace: false,
                    },
                    None,
                ));
            }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (unquote(k.trim()), v.trim());
        match (&table, pending.as_mut()) {
            (Some(_), Some((decl, crate_name))) => match k {
                "version" => decl.req = Some(unquote(v).to_string()),
                "package" => *crate_name = Some(unquote(v).to_string()),
                "workspace" => decl.workspace = v == "true",
                _ => {}
            },
            (Some((kind, target, None)), None) => {
                // `key = "1.0"`, `key = { ... }`, or dotted `key.workspace = true`.
                let (key, field) = match k.split_once('.') {
                    Some((key, field)) => (unquote(key), Some(field.trim())),
                    None => (k, None),
                };
                let mut decl = CargoDecl {
                    kind: *kind,
                    target: target.clone(),
                    key: key.to_string(),
                    req: None,
                    workspace: false,
                };
                let mut crate_name = None;
                match field {
                    Some("workspace") => decl.workspace = v == "true",
                    Some("version") => decl.req = Some(unquote(v).to_string()),
                    Some("package") => crate_name = Some(unquote(v).to_string()),
                    Some(_) => continue,
                    None if v.starts_with('{') => {
                        let body = v.trim_start_matches('{').trim_end_matches('}');
                        for kv in body.split(',') {
                            let Some((fk, fv)) = kv.split_once('=') else {
                                continue;
                            };
                            match fk.trim() {
                                "version" => decl.req = Some(unquote(fv.trim()).to_string()),
                                "package" => crate_name = Some(unquote(fv.trim()).to_string()),
                                "workspace" => decl.workspace = fv.trim() == "true",
                                _ => {}
                            }
                        }
                    }
                    None => decl.req = Some(unquote(v).to_string()),
                }
                if crate_key(crate_name.as_deref().unwrap_or(key)) != want {
                    continue;
                }
                // A dotted key can repeat over lines (`foo.version`, `foo.features`).
                if let Some(existing) = out
                    .iter_mut()
                    .find(|d| d.key == decl.key && d.kind == decl.kind && d.target == decl.target)
                {
                    existing.workspace |= decl.workspace;
                    if decl.req.is_some() {
                        existing.req = decl.req;
                    }
                } else {
                    out.push(decl);
                }
            }
            _ => {}
        }
    }
    flush(&mut pending, &mut out);
    out
}

/// `[dependencies]`, `[dev-dependencies.foo]`, `[target.'cfg(unix)'.build-dependencies]`, ...
fn cargo_table(header: &str) -> Option<(CargoKind, Option<String>, Option<String>)> {
    let inner = header.trim_start_matches('[').trim_end_matches(']').trim();
    let (target, rest) = match inner.strip_prefix("target.") {
        Some(rest) => {
            let (t, rest) = if let Some(q) = rest.strip_prefix(['\'', '"']) {
                let close = q.find(['\'', '"'])?;
                (&q[..close], q[close + 1..].trim_start_matches('.'))
            } else {
                rest.split_once('.')?
            };
            (Some(t.to_string()), rest)
        }
        None => (None, inner),
    };
    let (section, key) = match rest.split_once('.') {
        Some((s, k)) => (s, Some(unquote(k).to_string())),
        None => (rest, None),
    };
    let kind = match section {
        "dependencies" => CargoKind::Normal,
        "dev-dependencies" => CargoKind::Dev,
        "build-dependencies" => CargoKind::Build,
        _ => return None,
    };
    Some((kind, target, key))
}

/// The fix for one Cargo declaration.
pub fn cargo_direct(decl: &CargoDecl, package: &str, target: &str) -> Option<DirectFix> {
    if decl.workspace {
        return Some(DirectFix::Instruction(format!(
            "bump {package} to {target} in the workspace root's [workspace.dependencies] \
             (it is inherited here; `cargo add` in this crate would replace the inheritance \
             with a local pin)"
        )));
    }
    let op = match decl.req.as_deref().map(str::trim) {
        None => "",
        Some(r) if r.starts_with('=') => "=",
        Some(r) if r.starts_with('~') => "~",
        Some(r) if r.starts_with('^') => "^",
        Some(r) if r.starts_with(|c: char| c.is_ascii_digit()) => "",
        Some(_) => return None,
    };
    let spec = if op.is_empty() {
        format!("{package}@{target}")
    } else {
        format!("\"{package}@{op}{target}\"")
    };
    let mut cmd = format!("cargo add {spec}");
    match decl.kind {
        CargoKind::Normal => {}
        CargoKind::Dev => cmd.push_str(" --dev"),
        CargoKind::Build => cmd.push_str(" --build"),
    }
    if let Some(t) = &decl.target {
        cmd.push_str(&format!(" --target '{t}'"));
    }
    if crate_key(&decl.key) != crate_key(package) {
        cmd.push_str(&format!(" --rename {}", decl.key));
    }
    Some(DirectFix::Command(cmd))
}

/// `go get <module>@v<target>` when `go.mod` requires the module directly
/// (not `// indirect`).
pub fn go_direct_command(go_mod: &str, module: &str, target: &str) -> Option<String> {
    let mut in_block = false;
    let direct = go_mod.lines().any(|raw| {
        let line = raw.trim();
        if line.starts_with("require (") {
            in_block = true;
            return false;
        }
        if in_block && line.starts_with(')') {
            in_block = false;
            return false;
        }
        let entry = if in_block {
            Some(line)
        } else {
            line.strip_prefix("require ")
        };
        entry.is_some_and(|e| {
            !e.contains("// indirect") && e.split_whitespace().next() == Some(module)
        })
    });
    let version = if target.starts_with('v') {
        target.to_string()
    } else {
        format!("v{target}")
    };
    direct.then(|| format!("go get {module}@{version}"))
}

/// An exact edit for a `requirements.txt` line that pins the package with
/// `==`: no single verified command changes the file (`pip install` changes
/// only the environment).
pub fn pypi_direct_instruction(
    requirements: &str,
    file: &str,
    package: &str,
    target: &str,
) -> Option<String> {
    let norm = |s: &str| s.to_lowercase().replace(['_', '.'], "-");
    let want = norm(package);
    requirements.lines().find_map(|raw| {
        let line = raw.split('#').next().unwrap_or("").trim();
        let (name, version) = line.split_once("==")?;
        let name = name.split('[').next().unwrap_or(name).trim();
        (norm(name) == want && !version.trim().is_empty())
            .then(|| format!("change `{line}` to `{name}=={target}` in {file}"))
    })
}

fn unquote(s: &str) -> &str {
    s.trim().trim_matches('"').trim_matches('\'')
}

/// Cargo treats `-` and `_` in crate names as the same crate.
fn crate_key(name: &str) -> String {
    name.trim().to_lowercase().replace('_', "-")
}

#[cfg(test)]
#[path = "direct_upgrade_tests.rs"]
mod tests;

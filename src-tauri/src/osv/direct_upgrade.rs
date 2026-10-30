// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! How a DIRECT dependency moves to its fix target, worded for the tool and
//! the manifest that declares it (audit E8: a direct row said only
//! "4.2 -> 5.2.17").
//!
//! A command is named only where it was run against the tool and moved the
//! dependency while keeping it as declared (2026-10-15/16): npm 10.9.2, pnpm
//! 10.34.6, yarn 1.22.22, yarn 4.9.1, cargo 1.95 and go 1.24. Several naive
//! commands rewrote the declaration:
//! - `npm install x@1.2.8` writes `^1.2.8` over an exact pin and over `~1.2.5`;
//!   `--save-exact` or the declared operator keeps it;
//! - `yarn upgrade x@1.2.8` (yarn 1) and `yarn up x@1.2.8` (berry) write an
//!   exact pin over `^`/`~`; the operator in the spec keeps it;
//! - `cargo add` on a `workspace = true` dependency replaces the inheritance
//!   with a local pin, so the workspace root must move instead;
//! - `cargo add` on a dev or build dependency writes a second
//!   `[dependencies]` entry unless given `--dev` / `--build`.
//!
//! When the declared range already admits the target, the fix is the tool's
//! in-range lockfile refresh, which leaves the manifest as it is
//! ([`DirectFix::Refresh`], each form run 2026-10-18). Only a range bounded
//! below the next major (`^`, `~`, or a Cargo requirement) is refreshed: a
//! refresh installs the NEWEST release in range, which for `>=`, `||`, `*` or
//! an x-range could jump a major or land on a withdrawn or held release.
//!
//! Anything this cannot read with certainty names nothing, and the plan keeps
//! its plain "bump it where it is declared". That covers:
//! - a dependency declared twice;
//! - a declared spec that does not admit the installed copy;
//! - a git, path or alternate-registry source;
//! - a `>=`/`||`/`*`/x-range (neither a refresh nor an operator it can carry);
//! - a yarn workspace member (`yarn up` is project-wide);
//! - a Go `replace`;
//! - hashed or pip-compiled requirements;
//! - an unknown tool.

use std::path::Path;

use super::fix_path::{on_disk, requirement_admits, DetectManager, Manager, Requirement};
use super::fix_target::LineTarget;

/// What to tell the user, per declaring project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectFix {
    /// One command, run in the project's directory, that moves the declaration.
    Command(String),
    /// The declared range already admits the target: a lockfile refresh,
    /// run in the project's directory, moves the copy and leaves the
    /// manifest as it is (live 2026-10-18: navcal's sharp, next and vite were
    /// told to `npm install "x@^v"` though `^` already admitted the fix).
    Refresh(String),
    /// A manifest edit no single verified command makes.
    Instruction(String),
}

impl DirectFix {
    /// The text shown in the plan: a command in backticks, an instruction as is.
    pub fn render(&self) -> String {
        match self {
            Self::Command(c) | Self::Refresh(c) => format!("`{c}`"),
            Self::Instruction(i) => i.clone(),
        }
    }

    /// The text the Brief appends to "bump <pkg> to >= <to>: ".
    pub fn brief_clause(&self) -> String {
        match self {
            Self::Command(c) => format!("run `{c}`"),
            Self::Refresh(c) => format!("within its declared range, run `{c}`"),
            Self::Instruction(i) => i.clone(),
        }
    }
}

/// The in-range refresh of a DIRECT dependency, per tool: each was run on
/// 2026-10-18 against a project declaring `ms ^2.1.1` locked at 2.1.1 and
/// moved the lock to 2.1.3 with package.json unchanged (npm 10.9.2, pnpm
/// 10.34.6 also inside a workspace member, yarn 1.22.22, yarn 4.9.1; cargo
/// 1.97 kept `itoa = "1.0.5"` and locked 1.0.11). Plain `pnpm update x`
/// rewrote `^2.1.1` to `^2.1.3`, so pnpm needs `--no-save`.
fn direct_refresh_command(
    manager: Manager,
    package: &str,
    installed: &str,
    target: &str,
) -> String {
    match manager {
        Manager::Npm => format!("npm update {package}"),
        Manager::Pnpm | Manager::PnpmWorkspace => format!("pnpm update {package} --no-save"),
        Manager::YarnClassic => format!("yarn upgrade {package}"),
        Manager::YarnBerry => format!("yarn up -R {package}"),
        Manager::Cargo => format!("cargo update -p {package}@{installed} --precise {target}"),
    }
}

/// The fix for `package` at `installed`, declared in the project at
/// `project_dir`, read from its own manifest. `None` when it cannot be named
/// with certainty.
pub fn direct_fix(
    project_dir: &Path,
    ecosystem_norm: &str,
    package: &str,
    installed: &str,
    target: &str,
) -> Option<DirectFix> {
    // Findings carry the path lowercased; read the directory that is on disk.
    let project_dir = &on_disk(project_dir)?;
    let read = |file: &str| std::fs::read_to_string(project_dir.join(file)).ok();
    match ecosystem_norm {
        "npm" => npm_direct_fix(
            project_dir,
            &read("package.json")?,
            package,
            installed,
            target,
        ),
        "crates.io" => cargo_direct_fix(&read("Cargo.toml")?, package, installed, target),
        "Go" => {
            go_direct_command(&read("go.mod")?, package, installed, target).map(DirectFix::Command)
        }
        "PyPI" => [
            "requirements.txt",
            "requirements-dev.txt",
            "requirements/base.txt",
        ]
        .iter()
        .find_map(|f| pypi_direct_instruction(&read(f)?, f, package, installed, target))
        .map(DirectFix::Instruction),
        _ => None,
    }
}

/// The npm-family fix. A declaration that admits the target with an upper
/// bound below the next major (`^`, `~`) gets the refresh; one whose range
/// excludes the target gets the install that keeps its operator.
fn npm_direct_fix(
    project_dir: &Path,
    package_json: &str,
    package: &str,
    installed: &str,
    target: &str,
) -> Option<DirectFix> {
    let manager = Manager::detect(project_dir, "npm")?;
    // yarn's `up`/`upgrade` act on the whole workspace: run from a member
    // they rewrite siblings' declarations too.
    let yarn = matches!(manager, Manager::YarnBerry | Manager::YarnClassic);
    if yarn && !project_dir.join("yarn.lock").is_file() {
        return None;
    }
    let declared = npm_declared(package_json, package)?;
    if requirement_admits(&declared, installed) != Some(true) {
        return None;
    }
    let bounded = matches!(npm_operator(&declared), Some("^" | "~"));
    if bounded && requirement_admits(&declared, target) == Some(true) {
        return Some(DirectFix::Refresh(direct_refresh_command(
            manager, package, installed, target,
        )));
    }
    npm_direct_command(manager, &declared, package, target).map(DirectFix::Command)
}

/// The Cargo fix for the one declaration of `package`. A requirement that
/// admits the target gets `cargo update --precise`, which pins the exact
/// target; a workspace-inherited, git, path or registry entry never does.
fn cargo_direct_fix(
    manifest: &str,
    package: &str,
    installed: &str,
    target: &str,
) -> Option<DirectFix> {
    let decls = cargo_declarations(manifest, package);
    let [only] = decls.as_slice() else {
        return None;
    };
    if !cargo_admits(only, installed) {
        return None;
    }
    let admits_target = !only.workspace
        && !only.other_source
        && only
            .req
            .as_deref()
            .is_some_and(|r| Requirement::cargo(r).admits(target) == Some(true));
    if admits_target {
        Some(DirectFix::Refresh(direct_refresh_command(
            Manager::Cargo,
            package,
            installed,
            target,
        )))
    } else {
        cargo_direct(only, package, target)
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
    // A pre-release base is bounded too (decided for both engines): npm reads
    // `^2.0.0-rc.4` as `>=2.0.0-rc.4 <3.0.0-0`, exactly like `^2.0.0` above it
    // (NodeGoat's async 2.6.1, declared `^2.0.0-rc.4`).
    let base = |s: &str| digits(s) || prerelease_base(s);
    if let Some(rest) = d.strip_prefix('^') {
        return base(rest).then_some("^");
    }
    if let Some(rest) = d.strip_prefix('~') {
        return base(rest).then_some("~");
    }
    let exact = d.strip_prefix('=').unwrap_or(d).trim_start_matches('v');
    semver::Version::parse(exact).is_ok().then_some("")
}

/// The MCP's `PRERELEASE_BASE` (fix-paths.ts): `N.N.N-` and dot-separated
/// `[0-9A-Za-z-]+` identifiers. No `v`, no `+build`.
fn prerelease_base(s: &str) -> bool {
    let Some((core, pre)) = s.split_once('-') else {
        return false;
    };
    let nums: Vec<&str> = core.split('.').collect();
    nums.len() == 3
        && nums
            .iter()
            .all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        && pre
            .split('.')
            .all(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
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
    /// A `git`, `path` or `registry` source: `cargo add <crate>@<v>` would
    /// re-point it at crates.io.
    pub other_source: bool,
}

/// The fields of one declaration, folded in as they are read.
#[derive(Default)]
struct DeclFields {
    req: Option<String>,
    package: Option<String>,
    workspace: bool,
    other_source: bool,
}

impl DeclFields {
    fn set(&mut self, field: &str, value: &str) {
        match field {
            "version" => self.req = Some(unquote(value).to_string()),
            "package" => self.package = Some(unquote(value).to_string()),
            "workspace" => self.workspace |= value.trim() == "true",
            "git" | "path" | "registry" => self.other_source = true,
            _ => {}
        }
    }

    /// `{ version = "1", package = "x", ... }`.
    fn set_inline(&mut self, table: &str) {
        let body = table.trim().trim_start_matches('{').trim_end_matches('}');
        for kv in body.split(',') {
            if let Some((k, v)) = kv.split_once('=') {
                self.set(k.trim(), v.trim());
            }
        }
    }
}

/// Every declaration of `package` in a `Cargo.toml` (by crate name, so a
/// renamed dependency is found through its `package = ...`). Line-based, like
/// `fix_path::cargo_requirements`: a manifest this cannot read yields nothing.
pub fn cargo_declarations(manifest: &str, package: &str) -> Vec<CargoDecl> {
    let want = crate_key(package);
    // (kind, target, key, fields) per declaration, merged by (kind, target, key).
    let mut found: Vec<(CargoKind, Option<String>, String, DeclFields)> = Vec::new();
    let mut table: Option<(CargoKind, Option<String>, Option<String>)> = None;
    for raw in manifest.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            table = cargo_table(line);
            continue;
        }
        let (Some((kind, target, table_key)), Some((k, v))) = (&table, line.split_once('=')) else {
            continue;
        };
        let k = unquote(k.trim());
        // `[..dependencies.<key>]` fields, `<key>.<field> = ...`, or `<key> = ...`.
        let (key, field) = match table_key {
            Some(key) => (key.as_str(), Some(k)),
            None => match k.split_once('.') {
                Some((key, field)) => (unquote(key), Some(field.trim())),
                None => (k, None),
            },
        };
        let slot = match found
            .iter()
            .position(|(kd, t, ky, _)| kd == kind && t == target && ky == key)
        {
            Some(i) => &mut found[i].3,
            None => {
                found.push((
                    *kind,
                    target.clone(),
                    key.to_string(),
                    DeclFields::default(),
                ));
                &mut found.last_mut().expect("just pushed").3
            }
        };
        match field {
            Some(f) => slot.set(f, v),
            None if v.trim_start().starts_with('{') => slot.set_inline(v),
            None => slot.req = Some(unquote(v).to_string()),
        }
    }
    found
        .into_iter()
        .filter(|(_, _, key, f)| crate_key(f.package.as_deref().unwrap_or(key)) == want)
        .map(|(kind, target, key, f)| CargoDecl {
            kind,
            target,
            key,
            req: f.req,
            workspace: f.workspace,
            other_source: f.other_source,
        })
        .collect()
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

/// The declaration admits the installed copy (a workspace-inherited one is
/// judged at the root, so it passes here).
fn cargo_admits(decl: &CargoDecl, installed: &str) -> bool {
    match decl.req.as_deref() {
        Some(r) => Requirement::cargo(r).admits(installed) == Some(true),
        None => decl.workspace,
    }
}

/// The fix for one Cargo declaration.
pub fn cargo_direct(decl: &CargoDecl, package: &str, target: &str) -> Option<DirectFix> {
    if decl.other_source {
        return None;
    }
    if decl.workspace {
        return Some(DirectFix::Instruction(format!(
            "bump {package} to {target} in the workspace root's [workspace.dependencies] \
             (it is inherited here; `cargo add` in this crate would replace the inheritance \
             with a local pin)"
        )));
    }
    let op = match decl.req.as_deref().map(str::trim) {
        Some(r) if r.starts_with('=') => "=",
        Some(r) if r.starts_with('~') => "~",
        Some(r) if r.starts_with('^') => "^",
        Some(r) if r.starts_with(|c: char| c.is_ascii_digit()) => "",
        _ => return None,
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
        // Double quotes: cmd.exe keeps single quotes as part of the argument.
        cmd.push_str(&format!(" --target \"{t}\""));
    }
    if crate_key(&decl.key) != crate_key(package) {
        cmd.push_str(&format!(" --rename {}", decl.key));
    }
    Some(DirectFix::Command(cmd))
}

/// `go get <module>@v<target>` when `go.mod` requires the module directly at
/// `installed` (not `// indirect`), and no `replace` points it elsewhere.
pub fn go_direct_command(
    go_mod: &str,
    module: &str,
    installed: &str,
    target: &str,
) -> Option<String> {
    let v = |s: &str| {
        if s.starts_with('v') {
            s.to_string()
        } else {
            format!("v{s}")
        }
    };
    let (mut in_require, mut in_replace) = (false, false);
    let mut required_at: Option<String> = None;
    for raw in go_mod.lines() {
        let line = raw.trim();
        match line {
            "require (" => in_require = true,
            "replace (" => in_replace = true,
            ")" => (in_require, in_replace) = (false, false),
            _ => {}
        }
        let replaced = if in_replace {
            Some(line)
        } else {
            line.strip_prefix("replace ")
        };
        if replaced.is_some_and(|r| r.split_whitespace().next() == Some(module)) {
            return None;
        }
        let entry = if in_require {
            Some(line)
        } else {
            line.strip_prefix("require ")
        };
        if let Some(e) = entry.filter(|e| !e.contains("// indirect")) {
            let mut parts = e.split_whitespace();
            if parts.next() == Some(module) {
                required_at = parts.next().map(str::to_string);
            }
        }
    }
    (required_at? == v(installed)).then(|| format!("go get {module}@{}", v(target)))
}

/// An edit of the `requirements.txt` line that pins the package with `==` at
/// `installed`, replacing only the version (markers, extras and comments
/// stay). No single verified command changes the file (`pip install` changes
/// only the environment). Declined for hashed lines (`--require-hashes`
/// would fail) and for pip-compile output (edit its `.in` source instead).
pub fn pypi_direct_instruction(
    requirements: &str,
    file: &str,
    package: &str,
    installed: &str,
    target: &str,
) -> Option<String> {
    if requirements
        .lines()
        .take(8)
        .any(|l| l.contains("autogenerated by pip-compile"))
    {
        return None;
    }
    let norm = |s: &str| s.to_lowercase().replace(['_', '.'], "-");
    let want = norm(package);
    let lines: Vec<&str> = requirements.lines().collect();
    lines.iter().enumerate().find_map(|(i, raw)| {
        let line = raw.trim();
        let (head, rest) = line.split_once("==")?;
        let name = head.split('[').next().unwrap_or(head).trim();
        if norm(name) != want {
            return None;
        }
        let end = rest
            .find(|c: char| c == ';' || c == '\\' || c == '#' || c.is_whitespace())
            .unwrap_or(rest.len());
        let (version, tail) = rest.split_at(end);
        let continued = line.ends_with('\\')
            || line.contains("--hash")
            || lines
                .get(i + 1)
                .is_some_and(|n| n.trim_start().starts_with("--hash"));
        if continued || version != installed {
            return None;
        }
        Some(format!(
            "change `{line}` to `{head}=={target}{tail}` in {file}"
        ))
    })
}

/// Most declaring projects named in one step's direct-fix note.
const MAX_DIRECT_FIXES: usize = 3;

/// The direct copies' fixes, read from each declaring project's own manifest:
/// one per (project, target), in line order.
pub fn direct_fixes(
    ecosystem_norm: &str,
    package: &str,
    lines: &[LineTarget],
) -> Vec<(String, String, DirectFix)> {
    let mut out: Vec<(String, String, DirectFix)> = Vec::new();
    for line in lines {
        let Some(target) = line.target_version.as_deref() else {
            continue;
        };
        for site in line.sites.iter().filter(|s| s.is_direct) {
            if out
                .iter()
                .any(|(p, t, _)| *p == site.project_path && t == target)
            {
                continue;
            }
            if let Some(fix) = direct_fix(
                Path::new(&site.project_path),
                ecosystem_norm,
                package,
                &line.installed_version,
                target,
            ) {
                out.push((site.project_path.clone(), target.to_string(), fix));
            }
        }
    }
    out
}

/// Every DIRECT site of the row got an in-range lockfile refresh: the row is
/// worded as a refresh, not a bump. A site whose fix could not be named
/// (it needs an unnamed bump) keeps the bump wording (#993 review S2).
pub fn all_refresh(fixes: &[(String, String, DirectFix)], lines: &[LineTarget]) -> bool {
    let mut sites: Vec<(&str, &str)> = Vec::new();
    for line in lines {
        let Some(target) = line.target_version.as_deref() else {
            continue;
        };
        for site in line.sites.iter().filter(|s| s.is_direct) {
            if !sites.contains(&(site.project_path.as_str(), target)) {
                sites.push((site.project_path.as_str(), target));
            }
        }
    }
    !sites.is_empty()
        && sites.len() == fixes.len()
        && fixes
            .iter()
            .all(|(_, _, f)| matches!(f, DirectFix::Refresh(_)))
}

/// ": `npm install ...`" for one project, or each fix with the project it runs
/// in. In a row that mixes bumps and refreshes, a refresh says what it is.
pub fn direct_fix_note(fixes: &[(String, String, DirectFix)], multi_project: bool) -> String {
    if fixes.is_empty() {
        return String::new();
    }
    let mixed = !fixes
        .iter()
        .all(|(_, _, f)| matches!(f, DirectFix::Refresh(_)));
    let shown: Vec<String> = fixes
        .iter()
        .take(MAX_DIRECT_FIXES)
        .map(|(project, _, fix)| {
            let text = match fix {
                DirectFix::Refresh(_) if mixed => {
                    format!(
                        "{} (a lockfile refresh: the declared range already admits it)",
                        fix.render()
                    )
                }
                _ => fix.render(),
            };
            if multi_project {
                format!("{text} in {project}")
            } else {
                text
            }
        })
        .collect();
    let more = fixes.len().saturating_sub(MAX_DIRECT_FIXES);
    let tail = if more > 0 {
        format!(" (and {more} more)")
    } else {
        String::new()
    };
    format!(": {}{tail}", shown.join("; "))
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

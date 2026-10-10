// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Go: the module build list from `go.mod` (completed from `go.sum` before
//! Go 1.17).
//!
//! The old reader took EVERY version `go.sum` holds as installed. go.sum
//! keeps the hash of every module version the module graph ever visited —
//! historical, superseded and never-selected versions alike — so most of
//! them are not in the build. Measured 2026-10-10 against `go list -m all`
//! and govulncheck on caddy, hugo and traefik: precision 7.9%.
//!
//! What is installed is Go's build list (minimal version selection):
//! - **go >= 1.17**: go.mod's `require` list IS the build list (module-graph
//!   pruning makes go.mod name every module the build needs, `// indirect`
//!   ones included). go.sum is not consulted.
//! - **go < 1.17** (or no `go` directive, which Go reads as pre-1.17): go.mod
//!   names only direct requirements. A module the build needs but go.mod does
//!   not name is completed from go.sum: MVS selects the HIGHEST version the
//!   graph requires (go.sum records every one), and the module is built only
//!   if go.sum also hashes that version's source (the zip `h1:` line, not
//!   just the `/go.mod` line). A module with only a `/go.mod` line at its
//!   selected version was visited by the graph walk but never built. This
//!   deliberately goes beyond osv-scanner, which reads only go.mod's
//!   requirements for such modules and so misses most of the build
//!   (traefik, go 1.16: 83 of the 806 modules `go list -m all` reports).
//!
//! `replace` directives apply (a module replaced by another module version is
//! installed as that version; one replaced by a local path is local code and
//! dropped). `exclude` needs no handling: it only removes versions MVS would
//! otherwise consider, and the requirement lines already record the result.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{LockFormat, LockedPackage, LockfileRead};

/// A Go module version: `v1.2.3`, a pre-release, a pseudo-version, `+incompatible`.
fn is_go_version(v: &str) -> bool {
    let Some(rest) = v.strip_prefix('v') else {
        return false;
    };
    let core = rest.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// What a `replace` maps a module to: another module version, or local code.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Replacement {
    Module(String, String),
    Local,
}

/// Parsed go.mod: requirements (module -> version), replaces keyed by
/// `(module, Some(version) | None)`, and the `go` directive.
#[derive(Debug, Default)]
pub(super) struct GoMod {
    requires: BTreeMap<String, String>,
    replaces: HashMap<(String, Option<String>), Replacement>,
    go_version: Option<(u32, u32)>,
}

impl GoMod {
    /// Whether go.mod lists the full pruned build list (go >= 1.17).
    pub(super) fn lists_build_list(&self) -> bool {
        self.go_version.is_some_and(|v| v >= (1, 17))
    }

    fn replacement(&self, module: &str, version: &str) -> Option<&Replacement> {
        self.replaces
            .get(&(module.to_string(), Some(version.to_string())))
            .or_else(|| self.replaces.get(&(module.to_string(), None)))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    None,
    Require,
    Replace,
    Other,
}

pub(super) fn parse_go_mod(content: &str) -> GoMod {
    let mut out = GoMod::default();
    let mut block = Block::None;
    for raw in content.lines() {
        let line = raw.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if block != Block::None {
            if line == ")" {
                block = Block::None;
                continue;
            }
            directive(&mut out, block, line);
            continue;
        }
        let (keyword, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        let kind = match keyword {
            "require" => Block::Require,
            "replace" => Block::Replace,
            "go" => {
                out.go_version = parse_go_directive(rest);
                continue;
            }
            _ => Block::Other,
        };
        if rest == "(" {
            block = kind;
        } else {
            directive(&mut out, kind, rest);
        }
    }
    out
}

fn parse_go_directive(rest: &str) -> Option<(u32, u32)> {
    let mut parts = rest.split('.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor_text: String = parts
        .next()?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    Some((major, minor_text.parse().ok()?))
}

fn directive(out: &mut GoMod, kind: Block, line: &str) {
    match kind {
        Block::Require => {
            let mut tokens = line.split_whitespace();
            if let (Some(module), Some(version)) = (tokens.next(), tokens.next()) {
                if is_go_version(version) {
                    out.requires.insert(module.to_string(), version.to_string());
                }
            }
        }
        Block::Replace => {
            let Some((left, right)) = line.split_once("=>") else {
                return;
            };
            let left: Vec<&str> = left.split_whitespace().collect();
            let right: Vec<&str> = right.split_whitespace().collect();
            let Some(module) = left.first() else {
                return;
            };
            let target = match (right.first(), right.get(1)) {
                (Some(m), Some(v)) if is_go_version(v) => {
                    Replacement::Module(m.to_string(), v.to_string())
                }
                _ => Replacement::Local,
            };
            let key = (module.to_string(), left.get(1).map(|v| v.to_string()));
            out.replaces.insert(key, target);
        }
        Block::None | Block::Other => {}
    }
}

/// Per module, the version the build list selects from `go.sum` — the
/// HIGHEST version recorded there, by either line — when go.sum also hashes
/// that version's SOURCE (the zip line `module v1.2.3 h1:…`).
///
/// A `module v1.2.3/go.mod h1:…` line alone is not a build: the go command
/// records the go.mod hash of every module version it visits while walking
/// the module graph, but downloads (and hashes) the source only of modules
/// whose packages the build imports. Counting graph-only modules read 728
/// never-built modules as installed on hugo, node_exporter and traefik
/// (conformance corpus, 2026-10-10; truth `go list -m all` restricted to
/// source-hashed modules, the set govulncheck-style tools treat as built).
///
/// The selection is still the highest version overall: MVS picks the
/// highest version the graph requires, and an older version's leftover zip
/// hash does not make that older version built. node_exporter's go.sum
/// hashes `gogo/protobuf v1.1.1` source while the graph selects v1.2.1
/// (go.mod hash only) — the module is not built at either version.
pub(super) fn highest_in_go_sum(content: &str) -> BTreeMap<String, String> {
    let mut highest: BTreeMap<&str, &str> = BTreeMap::new();
    let mut source_hashed: HashSet<(&str, &str)> = HashSet::new();
    for line in content.lines() {
        let mut tokens = line.split_whitespace();
        let (Some(module), Some(raw)) = (tokens.next(), tokens.next()) else {
            continue;
        };
        let version = match raw.strip_suffix("/go.mod") {
            Some(v) => v,
            None => {
                source_hashed.insert((module, raw));
                raw
            }
        };
        if !is_go_version(version) {
            continue;
        }
        let higher = highest
            .get(module)
            .is_none_or(|current| go_order(version, current).is_gt());
        if higher {
            highest.insert(module, version);
        }
    }
    highest
        .into_iter()
        .filter(|(module, version)| source_hashed.contains(&(*module, *version)))
        .map(|(module, version)| (module.to_string(), version.to_string()))
        .collect()
}

/// Semver precedence over two Go versions; build metadata ignored.
fn go_order(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| crate::osv::version_order::OrderedVersion::parse(v);
    match (parse(a), parse(b)) {
        (Some(x), Some(y)) => x.compare(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => std::cmp::Ordering::Equal,
    }
}

/// The installed module set for the go.mod at `path` (content given).
pub(super) fn read_go_module(path: &Path, content: &str) -> LockfileRead {
    let module = parse_go_mod(content);
    let mut selected: BTreeMap<String, String> = module.requires.clone();
    if !module.lists_build_list() {
        if let Ok(sum) = std::fs::read_to_string(path.with_file_name("go.sum")) {
            for (name, version) in highest_in_go_sum(&sum) {
                selected.entry(name).or_insert(version);
            }
        }
    }
    let mut read = LockfileRead {
        path: PathBuf::new(),
        format: LockFormat::GoMod,
        packages: Vec::new(),
        edges: Vec::new(),
        non_registry_entries: 0,
    };
    let mut installed: BTreeMap<String, String> = BTreeMap::new();
    for (name, version) in selected {
        match module.replacement(&name, &version) {
            Some(Replacement::Local) => read.non_registry_entries += 1,
            Some(Replacement::Module(to, to_version)) => {
                installed.insert(to.clone(), to_version.clone());
            }
            None => {
                installed.entry(name).or_insert(version);
            }
        }
    }
    read.packages = installed
        .into_iter()
        .map(|(name, version)| {
            LockedPackage::new(name, version.trim_start_matches('v')).primary(true)
        })
        .collect();
    read
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared rule vectors — the drift-killer between the two dependency engines
//! until there is one (AD-054 decision 4).
//!
//! Runs every line of `src-tauri/tests/conformance/vectors/*.jsonl` against the
//! REAL functions: `version_order`, the matcher's window check and the
//! fix-target walk (all in `crates/fourda-depengine`), then
//! `fix_path::refresh_verdict` and the refresh commands. The TypeScript
//! MCP engine runs the same files. Expected values come from stated truth
//! (OSV's own answers, the 2026-10-10 fix-path oracle, PEP 440 / SemVer / Go
//! spec examples, AD-055), never from this engine; `vectors/README.md` maps
//! each rule to the ids that pin it.
//!
//! `vectors/engines_skip.json` (`rust` section) lists the ids this engine does
//! not satisfy yet, each with a reason. The gate is shrink-only:
//! - a vector that fails and is not skipped fails the test;
//! - a skipped vector that now PASSES fails the test (remove it from the list);
//! - the list may not grow past [`RUST_SKIP_BASELINE`], and when it shrinks the
//!   baseline must come down with it;
//! - a skip id that names no vector fails the test.
//!
//! Offline and fast: the only inputs are the vector files and the pinned
//! `osv/` records they name.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;

// version_order, osv_range and fix_target run against the pure engine crate
// directly (single-engine C0.1); refresh_verdict and fix_command are still the
// app's `fix_path` (C0.4), and records are merged by the app's `osv::sync`.
use fourda_depengine::fix_target::{fix_for_version, walk_target};
use fourda_depengine::range::{check_version_affected, parse_version};
use fourda_depengine::version_order::OrderedVersion;

use super::fix_path::{refresh_verdict, Basis, Manager, Refresh, Requirement};
use super::matching::package_key;
use super::types::Vulnerability;

/// The committed size of the `rust` skip list. Shrink-only: lower it in the
/// same change that removes entries; it may never rise.
const RUST_SKIP_BASELINE: usize = 28;

const FILES: [&str; 6] = [
    "version_order",
    "osv_range",
    "fix_target",
    "refresh_verdict",
    "fix_command",
    "range_check",
];

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
}

fn load_vectors(file: &str) -> Result<Vec<Value>, String> {
    let path = conformance_dir()
        .join("vectors")
        .join(format!("{file}.jsonl"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("vectors: read {}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str(l)
                .map_err(|e| format!("vectors: {file}.jsonl line {}: {e}", i + 1))
        })
        .collect()
}

fn rust_skips() -> Result<BTreeMap<String, String>, String> {
    let path = conformance_dir().join("vectors").join("engines_skip.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("vectors: {e}"))?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("vectors: engines_skip.json: {e}"))?;
    Ok(doc
        .get("rust")
        .and_then(Value::as_object)
        .ok_or("vectors: engines_skip.json has no `rust` object")?
        .iter()
        .map(|(id, reason)| (id.clone(), reason.as_str().unwrap_or_default().to_string()))
        .collect())
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn opt_s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

// ============================================================================
// Per-file checks: Ok(()) when the engine agrees with the vector.
// ============================================================================

fn check_version_order(v: &Value) -> Result<(), String> {
    let (a, b) = (s(v, "a"), s(v, "b"));
    let want = match s(v, "cmp") {
        "<" => Ordering::Less,
        "=" => Ordering::Equal,
        ">" => Ordering::Greater,
        other => return Err(format!("bad cmp {other:?}")),
    };
    let eco = s(v, "eco");
    let got = OrderedVersion::parse_in(a, eco)
        .zip(OrderedVersion::parse_in(b, eco))
        .and_then(|(x, y)| x.compare(&y));
    match got {
        Some(o) if o == want => Ok(()),
        Some(o) => Err(format!("{a} vs {b}: got {o:?}, want {want:?}")),
        None => Err(format!("{a} vs {b}: no ordering reads both")),
    }
}

/// Record id -> parsed record, loaded once per run.
#[derive(Default)]
struct Records(HashMap<String, Vulnerability>);

impl Records {
    /// The record's ranges for one package, merged exactly as `osv::sync`
    /// stores them (the JSON `check_version_affected` reads).
    fn ranges(&mut self, id: &str, eco: &str, name: &str) -> Result<Option<String>, String> {
        if !self.0.contains_key(id) {
            let path = conformance_dir().join("osv").join(format!("{id}.json"));
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("record {id} not pinned: {e}"))?;
            let vuln: Vulnerability =
                serde_json::from_str(&text).map_err(|e| format!("record {id}: {e}"))?;
            self.0.insert(id.to_string(), vuln);
        }
        let want = package_key(name, eco);
        let merged = super::sync::merge_affected(&self.0[id]);
        let hit = merged
            .into_iter()
            .find(|((n, e), _)| e == eco && package_key(n, e) == want)
            .ok_or_else(|| format!("record {id} names no {eco} {name}"))?;
        Ok(super::sync::serialize_ranges(&hit.1))
    }
}

fn check_osv_range(v: &Value, records: &mut Records) -> Result<(), String> {
    let ranges = records.ranges(s(v, "record_id"), s(v, "eco"), s(v, "name"))?;
    let want = v.get("affected").and_then(Value::as_bool).unwrap_or(false);
    match check_version_affected(s(v, "eco"), Some(s(v, "version")), &ranges) {
        (got, true) if got == want => Ok(()),
        (got, true) => Err(format!("affected: got {got}, want {want}")),
        (_, false) => Err("unconfirmed (the engine could not decide the window)".into()),
    }
}

/// Equal as versions. Build metadata is ignored whatever the ecosystem (the
/// README's convention), so a PyPI local label is compared away here too.
fn same_version(eco: &str, a: Option<&str>, b: Option<&str>) -> bool {
    let strip = |v: &str| v.split('+').next().unwrap_or(v).to_string();
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x == y
                || OrderedVersion::parse_in(&strip(x), eco)
                    .zip(OrderedVersion::parse_in(&strip(y), eco))
                    .and_then(|(p, q)| p.compare(&q))
                    == Some(Ordering::Equal)
        }
        _ => false,
    }
}

/// The copy's target as `fix_target::line_targets` forms it: where the walk
/// lands (`clears_all_known` when it clears everything), else, when the walk
/// cannot run, the highest own-line fix among the advisories that affect the
/// copy.
fn engine_target(eco: &str, installed: &str, ranges: &[Option<String>]) -> (Option<String>, bool) {
    let refs: Vec<&Option<String>> = ranges.iter().collect();
    if let Some(walk) = walk_target(eco, installed, &refs) {
        return (Some(walk.version), walk.clears_all);
    }
    let partial = ranges
        .iter()
        .filter(|r| check_version_affected(eco, Some(installed), r) == (true, true))
        .filter_map(|r| fix_for_version(eco, installed, r))
        .filter_map(|f| parse_version(&f).map(|p| (p, f)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, f)| f);
    (partial, false)
}

fn check_fix_target(v: &Value, records: &mut Records) -> Result<(), String> {
    let (eco, name, installed) = (s(v, "eco"), s(v, "name"), s(v, "installed"));
    let mut ranges = Vec::new();
    for id in v
        .get("record_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        ranges.push(records.ranges(id.as_str().unwrap_or_default(), eco, name)?);
    }
    let (target, clears) = engine_target(eco, installed, &ranges);
    let want_target = opt_s(v, "target");
    let want_clears = v.get("clears").and_then(Value::as_bool).unwrap_or(false);
    if !same_version(eco, target.as_deref(), want_target) {
        return Err(format!("target: got {target:?}, want {want_target:?}"));
    }
    // `clears` is only meaningful for a target.
    if want_target.is_some() && clears != want_clears {
        return Err(format!("clears: got {clears}, want {want_clears}"));
    }
    Ok(())
}

fn check_refresh_verdict(v: &Value) -> Result<(), String> {
    let reqs: Vec<(String, Requirement)> = v
        .get("requirements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|r| {
            let text = s(r, "text");
            let req = if s(r, "dialect") == "cargo" {
                Requirement::cargo(text)
            } else {
                Requirement::npm(text)
            };
            (s(r, "parent").to_string(), req)
        })
        .collect();
    let only: Vec<Requirement> = reqs.iter().map(|(_, r)| r.clone()).collect();
    let (target, installed) = (s(v, "target"), s(v, "installed"));
    let (verdict, inferred) = match refresh_verdict(installed, target, &only) {
        Refresh::Enough(b) => ("refresh", b == Basis::Semver),
        Refresh::ParentMustMove(b) => ("parent_must_move", b == Basis::Semver),
        Refresh::Unknown => ("unknown", true),
    };
    let via: Vec<&str> = reqs
        .iter()
        .filter(|(_, r)| r.admits(target) == Some(false))
        .map(|(p, _)| p.as_str())
        .collect();
    let want_via: Vec<&str> = v
        .get("via")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let want_inferred = v.get("inferred").and_then(Value::as_bool).unwrap_or(false);
    if verdict != s(v, "verdict") || inferred != want_inferred || via != want_via {
        return Err(format!(
            "got ({verdict}, inferred {inferred}, via {via:?}), want ({}, inferred {want_inferred}, via {want_via:?})",
            s(v, "verdict")
        ));
    }
    Ok(())
}

/// The files a project of this tool holds, for `Manager::detect`.
fn tool_files(tool: &str, workspace_root: bool) -> Vec<(&'static str, &'static str)> {
    let mut files = match tool {
        "npm" => vec![("package-lock.json", "{\"lockfileVersion\": 3}\n")],
        "pnpm" => vec![("pnpm-lock.yaml", "lockfileVersion: '6.0'\n")],
        "yarn1" => vec![("yarn.lock", "# yarn lockfile v1\n\n")],
        "yarn-berry" => vec![(
            "yarn.lock",
            "# This file is generated by running \"yarn install\"\n\n__metadata:\n  version: 6\n",
        )],
        "cargo" => vec![("Cargo.lock", "version = 3\n")],
        "pip" => vec![("requirements.txt", "jinja2==1.0.0\n")],
        "poetry" => vec![("poetry.lock", "[metadata]\n")],
        "uv" => vec![("uv.lock", "version = 1\n")],
        "pipenv" => vec![("Pipfile.lock", "{}\n")],
        "go" => vec![("go.mod", "module example.com/m\n"), ("go.sum", "")],
        "bundler" => vec![("Gemfile.lock", "GEM\n")],
        "composer" => vec![("composer.lock", "{}\n")],
        _ => Vec::new(),
    };
    if workspace_root {
        match tool {
            "pnpm" => files.push(("pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n")),
            "npm" => files.push(("package.json", "{\"workspaces\": [\"packages/*\"]}\n")),
            "cargo" => files.push(("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n")),
            _ => {}
        }
    }
    files
}

fn check_fix_command(v: &Value) -> Result<(), String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let workspace_root = v
        .get("workspace_root")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for (name, body) in tool_files(s(v, "tool"), workspace_root) {
        std::fs::write(dir.path().join(name), body).map_err(|e| e.to_string())?;
    }
    let manager = Manager::detect(dir.path(), s(v, "eco"));
    // `kind: "direct"` (audit E8): the command that moves a DECLARED
    // dependency; otherwise the refresh of a transitive copy (AD-055 ¶4).
    let command = if v.get("kind").and_then(Value::as_str) == Some("direct") {
        let dev = v.get("dev").and_then(Value::as_bool).unwrap_or(false);
        manager.map(|m| m.direct_command(s(v, "pkg"), s(v, "target"), dev))
    } else {
        manager.and_then(|m| m.refresh_command(s(v, "pkg"), s(v, "installed"), s(v, "target")))
    };
    let want = opt_s(v, "command");
    if command.as_deref() == want {
        Ok(())
    } else {
        Err(format!("command: got {command:?}, want {want:?}"))
    }
}

/// No Rust equivalent of the MCP's `range_check` (a `dependency_check` surface): every
/// `range_check` vector stays in the skip list until one exists.
fn check_range_check(_: &Value) -> Result<(), String> {
    Err("not implemented in Rust".into())
}

// ============================================================================
// The gate
// ============================================================================

#[test]
fn rule_vectors_hold_and_the_skip_list_only_shrinks() {
    let skips = rust_skips().expect("vectors: engines_skip.json");
    let mut records = Records::default();
    let mut problems: Vec<String> = Vec::new();
    let mut known_ids: BTreeMap<String, &str> = BTreeMap::new();
    let mut report: Vec<String> = Vec::new();
    for file in FILES {
        let vectors = load_vectors(file).expect("vectors: readable jsonl");
        let (mut pass, mut skipped) = (0usize, 0usize);
        for v in &vectors {
            let id = s(v, "id").to_string();
            assert!(
                !id.is_empty(),
                "vectors: {file}.jsonl has a line without an id"
            );
            assert!(
                known_ids.insert(id.clone(), file).is_none(),
                "vectors: duplicate id {id}"
            );
            assert!(
                !s(v, "rule").is_empty() && !s(v, "source").is_empty(),
                "vectors: {id} needs a `rule` and a `source`"
            );
            let outcome = match file {
                "version_order" => check_version_order(v),
                "osv_range" => check_osv_range(v, &mut records),
                "fix_target" => check_fix_target(v, &mut records),
                "refresh_verdict" => check_refresh_verdict(v),
                "fix_command" => check_fix_command(v),
                _ => check_range_check(v),
            };
            match (outcome, skips.contains_key(&id)) {
                (Ok(()), false) => pass += 1,
                (Ok(()), true) => problems.push(format!(
                    "{id} now PASSES — remove it from engines_skip.json (rust) and lower RUST_SKIP_BASELINE"
                )),
                (Err(_), true) => skipped += 1,
                (Err(why), false) => problems.push(format!("{id} ({file}): {why}")),
            }
        }
        report.push(format!(
            "{file}: {} vectors, {pass} pass, {skipped} skipped",
            vectors.len()
        ));
    }
    for id in skips.keys() {
        if !known_ids.contains_key(id) {
            problems.push(format!("skip entry {id} names no vector"));
        }
    }
    for (id, reason) in &skips {
        if reason.trim().is_empty() {
            problems.push(format!("skip entry {id} has no reason"));
        }
    }
    if skips.len() > RUST_SKIP_BASELINE {
        problems.push(format!(
            "the rust skip list grew to {} (baseline {RUST_SKIP_BASELINE}): it is shrink-only",
            skips.len()
        ));
    } else if skips.len() < RUST_SKIP_BASELINE {
        problems.push(format!(
            "the rust skip list shrank to {}: lower RUST_SKIP_BASELINE to match",
            skips.len()
        ));
    }
    println!("rule vectors (rust):\n  {}", report.join("\n  "));
    assert!(
        problems.is_empty(),
        "rule vectors:\n  {}",
        problems.join("\n  ")
    );
}

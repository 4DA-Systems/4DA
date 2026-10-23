// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Shared rule vectors — the drift-killer between the two dependency engines
//! until there is one (AD-054 decision 4).
//!
//! Runs every line of `src-tauri/tests/conformance/vectors/*.jsonl` against the
//! REAL functions: `version_order`, the matcher's window check and the
//! fix-target walk (all in `crates/fourda-depengine`), then
//! `fix_path::refresh_verdict`, the refresh commands and the severity rule
//! (`osv::severity`). The TypeScript
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
use fourda_depengine::fix_target::{installable_line_fix, walk_target_kinds, AdvisoryKind};
use fourda_depengine::range::{check_version_affected, parse_version};
use fourda_depengine::registry::RegistryReleases;
use fourda_depengine::version_order::OrderedVersion;

use super::fix_path::{refresh_verdict, Basis, Manager, Refresh, Requirement};
use super::matching::package_key;
use super::types::{MatchedAdvisory, StoredAdvisory, Vulnerability};

/// The committed size of the `rust` skip list. Shrink-only: lower it in the
/// same change that removes entries; it may never rise.
const RUST_SKIP_BASELINE: usize = 28;

const FILES: [&str; 9] = [
    "version_order",
    "osv_range",
    "fix_target",
    "refresh_verdict",
    "fix_command",
    "range_check",
    "holding",
    "parent_release",
    "severity",
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

    /// How the walk treats the record for one package (malware, maintenance
    /// notice), classified by the pure crate exactly as the conformance path
    /// stores it (`fourda_depengine::osv::stored_records`).
    fn kind(id: &str, eco: &str, name: &str) -> Result<AdvisoryKind, String> {
        let path = conformance_dir().join("osv").join(format!("{id}.json"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("record {id} not pinned: {e}"))?;
        let rec: fourda_depengine::osv::OsvRecord =
            serde_json::from_str(&text).map_err(|e| format!("record {id}: {e}"))?;
        let want = package_key(name, eco);
        fourda_depengine::osv::stored_records(&rec)
            .into_iter()
            .find(|r| r.ecosystem == eco && package_key(&r.package_name, &r.ecosystem) == want)
            .map(|r| r.kind())
            .ok_or_else(|| format!("record {id} names no {eco} {name}"))
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

/// npm's security holding placeholder (`osv/matching`): a `*-security`
/// release of an npm name has no code, so no advisory applies to it, whatever
/// database's id flags the name.
fn check_holding(v: &Value) -> Result<(), String> {
    let got =
        fourda_depengine::matching::is_npm_security_holding(s(v, "eco"), Some(s(v, "version")));
    let want = v.get("holding").and_then(Value::as_bool).unwrap_or(false);
    if got == want {
        Ok(())
    } else {
        Err(format!("holding: got {got}, want {want}"))
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
/// copy. Neither is ever a release `registry` withdrew; the third value names
/// the withdrawn ones the walk stepped over; the fourth, the indices of the
/// advisories every fixing release brings in (then there is no target, not
/// even a line fix: parity decision 2); the fifth, with no target, the
/// malicious records a fixing release falls in (the MCP's `maliciousLanding`).
fn engine_target(
    eco: &str,
    installed: &str,
    ranges: &[Option<String>],
    kinds: &[AdvisoryKind],
    registry: &RegistryReleases,
) -> EngineTarget {
    let refs: Vec<&Option<String>> = ranges.iter().collect();
    let outcome = walk_target_kinds(eco, installed, &refs, kinds, registry);
    if let Some(walk) = outcome.target {
        return (
            Some(walk.version),
            walk.clears_all,
            outcome.withheld,
            Vec::new(),
            Vec::new(),
        );
    }
    if !outcome.introduces.is_empty() || !outcome.malware.is_empty() {
        return (
            None,
            false,
            outcome.withheld,
            outcome.introduces,
            outcome.malware,
        );
    }
    // A maintenance notice is never a target, not even its own line fix.
    let partial = ranges
        .iter()
        .zip(
            kinds
                .iter()
                .copied()
                .chain(std::iter::repeat(AdvisoryKind::default())),
        )
        .filter(|(_, k)| !k.informational)
        .map(|(r, _)| r)
        .filter(|r| check_version_affected(eco, Some(installed), r) == (true, true))
        .filter_map(|r| installable_line_fix(eco, installed, r, registry).fix)
        .filter_map(|f| parse_version(&f).map(|p| (p, f)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, f)| f);
    (partial, false, outcome.withheld, Vec::new(), Vec::new())
}

/// (target, clears, withheld, introduced indices, malicious indices).
type EngineTarget = (Option<String>, bool, Vec<String>, Vec<usize>, Vec<usize>);

/// A string-array field of a vector line (absent reads as empty).
fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn check_fix_target(v: &Value, records: &mut Records) -> Result<(), String> {
    let (eco, name, installed) = (s(v, "eco"), s(v, "name"), s(v, "installed"));
    let mut ranges = Vec::new();
    let mut kinds = Vec::new();
    let ids = strings(v, "record_ids");
    for id in &ids {
        kinds.push(Records::kind(id, eco, name)?);
    }
    for id in v
        .get("record_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        ranges.push(records.ranges(id.as_str().unwrap_or_default(), eco, name)?);
    }
    let registry = RegistryReleases {
        published: strings(v, "published"),
        excluded: strings(v, "excluded"),
    };
    let (target, clears, withheld, introduced, malware) =
        engine_target(eco, installed, &ranges, &kinds, &registry);
    // `malicious` (record ids, sorted) is checked only on lines that state it.
    let mut malicious: Vec<String> = malware
        .iter()
        .filter_map(|&i| ids.get(i).cloned())
        .collect();
    malicious.sort();
    malicious.dedup();
    if v.get("malicious").is_some() && malicious != strings(v, "malicious") {
        return Err(format!(
            "malicious: got {malicious:?}, want {:?}",
            strings(v, "malicious")
        ));
    }
    // `introduces` (record ids) is checked only on lines that state it.
    // Sorted and deduped, as both engines name them.
    let mut introduced: Vec<String> = introduced
        .iter()
        .filter_map(|&i| ids.get(i).cloned())
        .collect();
    introduced.sort();
    introduced.dedup();
    if v.get("introduces").is_some() && introduced != strings(v, "introduces") {
        return Err(format!(
            "introduces: got {introduced:?}, want {:?}",
            strings(v, "introduces")
        ));
    }
    let want_target = opt_s(v, "target");
    let want_clears = v.get("clears").and_then(Value::as_bool).unwrap_or(false);
    if !same_version(eco, target.as_deref(), want_target) {
        return Err(format!("target: got {target:?}, want {want_target:?}"));
    }
    // `clears` is only meaningful for a target.
    if want_target.is_some() && clears != want_clears {
        return Err(format!("clears: got {clears}, want {want_clears}"));
    }
    // `withheld` is checked only on lines that state it.
    if v.get("withheld").is_some() && withheld != strings(v, "withheld") {
        return Err(format!(
            "withheld: got {withheld:?}, want {:?}",
            strings(v, "withheld")
        ));
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
            } else if r.get("bundled").and_then(Value::as_bool) == Some(true) {
                // A bundled copy: the owner's tarball pins the shipped version,
                // spelled as the scanner stores it (audit E8).
                Requirement::npm(&format!("{text}{}", crate::osv::fix_path::BUNDLED_SUFFIX))
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
    // `override` (optional): whether an override may be offered for this
    // copy — never when any blocking parent ships it bundled (audit E8).
    if let Some(want_override) = v.get("override").and_then(Value::as_bool) {
        let bundled = only
            .iter()
            .any(|r| crate::osv::fix_path::is_bundled(&r.text) && r.admits(target) == Some(false));
        let manager = (s(v, "eco") == "npm").then_some(crate::osv::fix_path::Manager::Npm);
        let offered = crate::osv::fix_path::override_hint(manager, s(v, "eco"), bundled).is_some();
        if offered != want_override {
            return Err(format!(
                "override offered: got {offered}, want {want_override}"
            ));
        }
    }
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
    if v.get("kind").and_then(Value::as_str) == Some("direct") {
        return check_direct(v, dir.path());
    }
    let command = Manager::detect(dir.path(), s(v, "eco"))
        .and_then(|m| m.refresh_command(s(v, "pkg"), s(v, "installed"), s(v, "target")));
    let want = opt_s(v, "command");
    if command.as_deref() == want {
        Ok(())
    } else {
        Err(format!("command: got {command:?}, want {want:?}"))
    }
}

/// `kind: "direct"` (audit E8): the fix for a DECLARED dependency, read by
/// `direct_fix` from a project built out of the vector: its declared npm spec
/// (`declared`), `Cargo.toml` (`manifest`), `go.mod` (`go_mod`) or
/// `requirements.txt` (`requirements`). `member: true` puts the manifest one
/// workspace level below the lockfile. A command is checked against
/// `command`, an instruction against `instruction`.
fn check_direct(v: &Value, root: &std::path::Path) -> Result<(), String> {
    use crate::osv::direct_upgrade::{direct_fix, DirectFix};
    let pkg = s(v, "pkg");
    let project = if v.get("member").and_then(Value::as_bool) == Some(true) {
        let member = root.join("packages").join("app");
        std::fs::create_dir_all(&member).map_err(|e| e.to_string())?;
        member
    } else {
        root.to_path_buf()
    };
    let mut files: Vec<(&str, String)> = Vec::new();
    if let Some(declared) = opt_s(v, "declared") {
        let pkg_json = serde_json::json!({ "dependencies": { pkg: declared } });
        files.push(("package.json", pkg_json.to_string()));
    }
    for (field, file) in [
        ("manifest", "Cargo.toml"),
        ("go_mod", "go.mod"),
        ("requirements", "requirements.txt"),
    ] {
        if let Some(body) = opt_s(v, field) {
            files.push((file, body.to_string()));
        }
    }
    for (file, body) in files {
        std::fs::write(project.join(file), body).map_err(|e| e.to_string())?;
    }
    let fix = direct_fix(
        &project,
        s(v, "eco"),
        pkg,
        s(v, "installed"),
        s(v, "target"),
    );
    let (command, instruction, refresh) = match &fix {
        Some(DirectFix::Command(c)) => (Some(c.as_str()), None, false),
        Some(DirectFix::Refresh(c)) => (Some(c.as_str()), None, true),
        Some(DirectFix::Instruction(i)) => (None, Some(i.as_str()), false),
        None => (None, None, false),
    };
    let (want_c, want_i) = (opt_s(v, "command"), opt_s(v, "instruction"));
    let want_refresh = v.get("refresh").and_then(Value::as_bool).unwrap_or(false);
    if command == want_c && instruction == want_i && refresh == want_refresh {
        Ok(())
    } else {
        Err(format!(
            "direct: got {fix:?}, want command {want_c:?} / instruction {want_i:?}"
        ))
    }
}

/// One record of a `severity` line as the matcher carries it: its own grade
/// read exactly as `osv::sync` stores it (best CVSS score, then the curated
/// label or the CVSS band), and the line's optional `fixed_version` (the
/// copy's fix, which tells a maintenance notice from a vulnerability).
fn severity_match(vuln: &Vulnerability, fixed_version: Option<&str>) -> MatchedAdvisory {
    let (severity_type, cvss_score) = vuln.best_cvss();
    MatchedAdvisory {
        advisory_id: vuln.id.clone(),
        summary: vuln.summary.clone().unwrap_or_default(),
        details: None,
        package_name: "pkg".into(),
        ecosystem: "crates.io".into(),
        installed_version: Some("1.0.0".into()),
        fixed_version: fixed_version.map(str::to_string),
        severity_type,
        cvss_score,
        source_url: None,
        is_version_confirmed: true,
        project_paths: vec!["/p".into()],
        published_at: None,
        dependency_instances: Vec::new(),
        aliases: vuln.aliases.clone().unwrap_or_default(),
        severity_label: vuln.severity_label(),
        severity_from: None,
        // `database_specific.cwe_ids`, as `osv::sync` stores it (CWE-506 is
        // how a GHSA malware advisory with no MAL- alias says so).
        cwe_ids: vuln
            .cwe_ids_json()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
    }
}

/// A mirror row for one `aliases_available` entry: its grade, or none.
fn mirror_row(id: &str, label: Option<&str>) -> StoredAdvisory {
    StoredAdvisory {
        id: 0,
        advisory_id: id.to_string(),
        summary: String::new(),
        details: None,
        package_name: "other".into(),
        ecosystem: "npm".into(),
        affected_ranges: None,
        fixed_versions: None,
        severity_type: None,
        cvss_score: None,
        source_url: None,
        published_at: None,
        modified_at: None,
        withdrawn_at: None,
        synced_at: String::new(),
        aliases: Vec::new(),
        severity_label: label.and_then(super::types::normalize_severity_label),
        cwe_ids: Vec::new(),
    }
}

/// `severity`: the grade one advisory (or one alias group, `records`) carries
/// on every surface, where it came from, and the base urgency both the
/// Upgrade Plan and the alerts give it (`osv::severity`).
fn check_severity(v: &Value) -> Result<(), String> {
    use super::severity::{cluster_severity, cluster_urgency, grade_unrated};
    let records: Vec<Value> = match v.get("record") {
        Some(r) => vec![r.clone()],
        None => v
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    };
    let mut matches = Vec::new();
    for r in records {
        let vuln: Vulnerability = serde_json::from_value(r).map_err(|e| format!("record: {e}"))?;
        matches.push(severity_match(&vuln, opt_s(v, "fixed_version")));
    }
    // The same records as mirror rows, for the path Knowledge Gaps and the
    // scorer grade by (`rows_grade`), before the matcher stores anything.
    let stored: Vec<StoredAdvisory> = matches
        .iter()
        .map(|m| StoredAdvisory {
            advisory_id: m.advisory_id.clone(),
            summary: m.summary.clone(),
            cvss_score: m.cvss_score,
            severity_label: m.severity_label.clone(),
            aliases: m.aliases.clone(),
            fixed_versions: m.fixed_version.as_ref().map(|f| format!("[\"{f}\"]")),
            ..mirror_row(&m.advisory_id, None)
        })
        .collect();
    let mirror: Vec<StoredAdvisory> = v
        .get("aliases_available")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(id, label)| mirror_row(id, label.as_str()))
        .collect();
    grade_unrated(&mut matches, &mirror);
    let refs: Vec<&MatchedAdvisory> = matches.iter().collect();
    if super::identity::cluster_by_vulnerability(&refs).len() != 1 {
        return Err("the records are not one alias group".into());
    }
    let graded = cluster_severity(&refs);
    let got = (
        graded.as_ref().map(|g| g.tier.to_string()),
        graded.and_then(|g| g.from),
        format!("{:?}", cluster_urgency(&refs)).to_lowercase(),
    );
    let expect = v.get("expect").ok_or("no `expect`")?;
    let want = (
        opt_s(expect, "severity").map(str::to_string),
        opt_s(expect, "severity_from").map(str::to_string),
        s(expect, "urgency").to_string(),
    );
    if got != want {
        return Err(format!(
            "got (severity, from, urgency) {got:?}, want {want:?}"
        ));
    }
    let rows: Vec<&StoredAdvisory> = stored.iter().collect();
    let by_rows =
        super::severity::rows_grade(&rows, &super::severity::AliasGrades::from_rows(&mirror));
    let got_rows = (
        by_rows.as_ref().map(|g| g.tier.to_string()),
        by_rows.and_then(|g| g.from),
    );
    if got_rows == (want.0.clone(), want.1.clone()) {
        Ok(())
    } else {
        Err(format!(
            "mirror-row path: got (severity, from) {got_rows:?}, want ({:?}, {:?})",
            want.0, want.1
        ))
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
                "holding" => check_holding(v),
                "severity" => check_severity(v),
                "fix_target" => check_fix_target(v, &mut records),
                "refresh_verdict" => check_refresh_verdict(v),
                "fix_command" => check_fix_command(v),
                "parent_release" => super::parent_release_vectors::check_parent_release(v),
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

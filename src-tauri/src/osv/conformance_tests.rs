// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dependency-engine conformance corpus — the hermetic G1 gate.
//!
//! Reads `src-tauri/tests/conformance/` (format: README.md there): ~35 cases
//! of vendored public lockfiles, the OSV records that name their packages
//! (pinned, as served by api.osv.dev), and an adjudicated truth set per case.
//! Offline and deterministic: the lockfiles go through the app's lockfile
//! readers into an in-memory DB, the pinned advisories go in exactly the way
//! `osv::sync` stores them (`store_vulnerability`), and the REAL matcher runs.
//!
//! The result is scored per ecosystem against `thresholds.json`, a RATCHET:
//! it records the values measured when the corpus landed, so CI stays green
//! and the numbers can only go up. Raise it in the same PR as a fix.
//!
//! Run `FOURDA_CONFORMANCE_DUMP=<dir> cargo test --lib osv::conformance_tests
//! -- --nocapture` to write the engine's per-case inventory and findings for
//! `scripts/conformance-corpus.mjs` (truth regeneration + adjudication report).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::db::{Database, DependencyInstanceInput};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
}

// ============================================================================
// Corpus format (mirrors README.md / scripts/conformance-corpus.mjs)
// ============================================================================

#[derive(Debug, Deserialize)]
struct CaseMeta {
    id: String,
    source: String,
    licence: String,
    formats: Vec<String>,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    commit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Expected {
    schema: u32,
    case: String,
    lockfiles: Vec<LockfileRow>,
    inventory: Inventory,
    findings: Vec<ExpectedFinding>,
}

#[derive(Debug, Deserialize)]
struct LockfileRow {
    path: String,
    /// The project directory the lockfile belongs to (a `requirements/*.txt`
    /// file belongs to the directory above it).
    dir: String,
    format: String,
    ecosystem: String,
    status: String,
    #[serde(default)]
    truth_packages: usize,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Inventory {
    count: usize,
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExpectedFinding {
    ecosystem: String,
    dir: String,
    package: String,
    version: String,
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Thresholds {
    ecosystems: BTreeMap<String, EcoThreshold>,
    max_silent_drops: usize,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct EcoThreshold {
    findings_precision: f64,
    findings_recall: f64,
    inventory_precision: f64,
    inventory_recall: f64,
}

struct Case {
    meta: CaseMeta,
    expected: Expected,
    dir: PathBuf,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("conformance: cannot read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("conformance: cannot parse {}: {e}", path.display()))
}

fn load_cases() -> Vec<Case> {
    let root = corpus_dir();
    let mut ids: Vec<String> = std::fs::read_dir(root.join("cases"))
        .expect("conformance: cases/ directory")
        .flatten()
        .filter(|e| e.path().join("case.json").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| Case {
            meta: read_json(&root.join("cases").join(&id).join("case.json")),
            expected: read_json(&root.join("expected").join(format!("{id}.json"))),
            dir: root.join("cases").join(&id),
        })
        .collect()
}

/// Every file under `dir`, as `/`-separated paths relative to it.
fn walk_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(dir) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// Files that hold (or claim to hold) an installed dependency set. Same rule
/// as `isLockfileLike` in scripts/conformance-corpus.mjs.
fn is_lockfile_like(rel: &str) -> bool {
    const NAMES: &[&str] = &[
        "package-lock.json",
        "npm-shrinkwrap.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
        "Cargo.lock",
        "poetry.lock",
        "uv.lock",
        "Pipfile.lock",
        "go.mod",
        "go.sum",
        "Gemfile.lock",
        "composer.lock",
        "pom.xml",
        "gradle.lockfile",
    ];
    let base = rel.rsplit('/').next().unwrap_or(rel);
    let lower = base.to_ascii_lowercase();
    NAMES.contains(&base)
        || (lower.starts_with("requirements") && lower.ends_with(".txt"))
        || (lower.ends_with(".txt") && rel.to_ascii_lowercase().contains("requirements/"))
}

// ============================================================================
// Key normalisation (identical rules in scripts/conformance-corpus.mjs)
// ============================================================================

fn norm_name(eco: &str, name: &str) -> String {
    match eco {
        "PyPI" => {
            let lower = name.to_lowercase();
            let mut out = String::with_capacity(lower.len());
            for ch in lower.chars() {
                let sep = matches!(ch, '-' | '_' | '.');
                if !(sep && out.ends_with('-')) {
                    out.push(if sep { '-' } else { ch });
                }
            }
            out
        }
        "Go" => name.to_string(),
        _ => name.to_lowercase(),
    }
}

fn norm_version(eco: &str, version: &str) -> String {
    let v = version.trim();
    let strip_v = |s: &str| -> String {
        match s.strip_prefix('v') {
            Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest.to_string(),
            _ => s.to_string(),
        }
    };
    match eco {
        "Go" | "Packagist" => strip_v(v),
        "PyPI" => {
            let v = v.to_lowercase();
            let v = v.strip_prefix('v').unwrap_or(&v);
            let (release, rest) = v.split_at(pep440_release_len(v));
            let mut parts: Vec<&str> = release.split('.').collect();
            while parts.len() > 1 && parts.last().is_some_and(|p| p.bytes().all(|c| c == b'0')) {
                parts.pop();
            }
            format!("{}{rest}", parts.join("."))
        }
        _ => v.to_string(),
    }
}

/// Length of the leading `\d+(\.\d+)*` release segment of a PEP 440 version.
fn pep440_release_len(v: &str) -> usize {
    let bytes = v.as_bytes();
    let mut end = 0;
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            break;
        }
        end = i;
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
        } else {
            break;
        }
    }
    end
}

fn inv_key(eco: &str, dir: &str, name: &str, version: &str) -> String {
    format!(
        "{eco}|{dir}|{}|{}",
        norm_name(eco, name),
        norm_version(eco, version)
    )
}

/// The ACE language string a lockfile processor stores -> the OSV ecosystem.
fn osv_ecosystem(language_or_eco: &str) -> String {
    match language_or_eco.to_ascii_lowercase().as_str() {
        "javascript" | "npm" => "npm".into(),
        "rust" | "crates.io" => "crates.io".into(),
        "python" | "pypi" => "PyPI".into(),
        "go" | "golang" => "Go".into(),
        "ruby" | "rubygems" => "RubyGems".into(),
        "php" | "packagist" => "Packagist".into(),
        other => other.to_string(),
    }
}

// ============================================================================
// Reading the corpus through the app's lockfile readers
// ============================================================================

/// Project path the corpus stores a case directory under. Synthetic, so it
/// never collides with a real project and passes the agent-infra guards.
fn project_path(case: &str, rel_dir: &str) -> String {
    if rel_dir == "." {
        format!("/conformance/{case}")
    } else {
        format!("/conformance/{case}/{rel_dir}")
    }
}

/// Feed every lockfile directory of one case to the readers: the lockfile
/// walk's own directory selection (`ace::lockfile::walk_dirs` — skip list and
/// depth, without the user-scope gates, which are policy, not parsing).
fn ingest_case(db: &Database, case: &Case) {
    for dir in crate::ace::lockfile::walk_dirs(&case.dir) {
        let rel = dir
            .strip_prefix(&case.dir)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let rel = if rel.is_empty() { ".".to_string() } else { rel };
        read_lockfile_dir(db, &dir, &project_path(&case.meta.id, &rel));
    }
}

/// What `ace_commands::dependencies::process_lockfile_dir` stores for one
/// directory: every lockfile read by `ace::lockfile::read_dir` (the same
/// readers, Cargo workspace crates already removed), merged per ecosystem and
/// written once — minus what cannot affect matching (direct/dev labels,
/// dependency edges, the `cargo tree` host probe, prunes). Failed and
/// unsupported outcomes store nothing; the silent-drop score sees them.
fn read_lockfile_dir(db: &Database, dir: &Path, project: &str) {
    use crate::ace::lockfile::{read_dir, LockfileOutcome};
    let mut by_ecosystem: BTreeMap<&'static str, Vec<(String, String)>> = BTreeMap::new();
    for outcome in read_dir(dir) {
        if let LockfileOutcome::Read(read) = outcome {
            let rows = by_ecosystem.entry(read.format.ecosystem()).or_default();
            rows.extend(read.packages.into_iter().map(|p| (p.name, p.version)));
        }
    }
    for (language, mut packages) in by_ecosystem {
        packages.sort();
        packages.dedup();
        store_packages(db, project, language, &packages);
    }
}

/// The two writes every processor makes: the multi-version instance
/// inventory, then the collapsed per-package rows the matcher indexes.
fn store_packages(db: &Database, project: &str, language: &str, packages: &[(String, String)]) {
    let instances: Vec<DependencyInstanceInput> = packages
        .iter()
        .map(|(name, version)| DependencyInstanceInput {
            package_name: name.clone(),
            version: version.clone(),
            is_direct: false,
            is_dev: false,
            scope: "unknown".to_string(),
        })
        .collect();
    db.store_dependency_instances(project, language, &instances)
        .expect("conformance: store instances");
    for (name, version) in packages {
        db.store_transitive_dependency(project, name, Some(version), language, false)
            .expect("conformance: store dependency");
    }
}

/// Every pinned record, stored the way `osv::sync` stores a fetched one.
/// Returns the alias union (id -> canonical id) used to group advisories.
fn load_pinned_advisories(db: &Database) -> HashMap<String, String> {
    let dir = corpus_dir().join("osv");
    let mut seen = std::collections::HashSet::new();
    let mut uf = AliasUnion::default();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("conformance: osv/ directory")
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    for path in files {
        let vuln: super::types::Vulnerability = read_json(&path);
        for alias in vuln.aliases.iter().flatten() {
            uf.union(&vuln.id, alias);
        }
        super::sync::store_vulnerability(db, &vuln, &mut seen)
            .unwrap_or_else(|e| panic!("conformance: store {}: {e}", vuln.id));
    }
    uf.canonical_map()
}

#[derive(Default)]
struct AliasUnion {
    parent: HashMap<String, String>,
}

impl AliasUnion {
    fn find(&mut self, x: &str) -> String {
        let mut path = Vec::new();
        let mut cur = x.to_string();
        loop {
            let next = self
                .parent
                .entry(cur.clone())
                .or_insert_with(|| cur.clone())
                .clone();
            if next == cur {
                break;
            }
            path.push(std::mem::replace(&mut cur, next));
        }
        for node in path {
            self.parent.insert(node, cur.clone());
        }
        cur
    }

    fn union(&mut self, a: &str, b: &str) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }

    /// id -> the group's readable representative: the smallest non-CVE id
    /// (an advisory database id such as GHSA-/GO-/PYSEC-), else the smallest.
    fn canonical_map(mut self) -> HashMap<String, String> {
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        let roots: Vec<(String, String)> = keys
            .into_iter()
            .map(|k| {
                let root = self.find(&k);
                (k, root)
            })
            .collect();
        let mut best: HashMap<&str, &str> = HashMap::new();
        for (id, root) in &roots {
            let rank = |s: &str| (s.starts_with("CVE-"), s.to_string());
            let slot = best.entry(root.as_str()).or_insert(id.as_str());
            if rank(id) < rank(slot) {
                *slot = id.as_str();
            }
        }
        roots
            .iter()
            .map(|(id, root)| (id.clone(), best[root.as_str()].to_string()))
            .collect()
    }
}

// ============================================================================
// Running the engine
// ============================================================================

/// What the engine reported for the whole corpus, keyed by case id.
#[derive(Default)]
struct EngineRun {
    /// case -> inventory keys (`eco|dir|name|version`)
    inventory: BTreeMap<String, BTreeSet<String>>,
    /// case -> confirmed finding keys (`eco|dir|name|version|canonical-id`)
    findings: BTreeMap<String, BTreeSet<String>>,
    /// case -> raw confirmed findings, for the dump
    raw_findings: BTreeMap<String, Vec<serde_json::Value>>,
    /// Matches the engine could not version-confirm (never shown as findings).
    unconfirmed: usize,
}

/// Map a stored project path back to (case, dir).
fn locate(
    path_index: &HashMap<String, (String, String)>,
    project: &str,
) -> Option<(String, String)> {
    let key = project.replace('\\', "/").to_lowercase();
    path_index.get(key.trim_end_matches('/')).cloned()
}

fn path_index(cases: &[Case]) -> HashMap<String, (String, String)> {
    let mut index = HashMap::new();
    for case in cases {
        let mut dirs: BTreeSet<String> = BTreeSet::from([".".to_string()]);
        for rel in walk_files(&case.dir) {
            let mut parent = rel.rsplit_once('/').map_or(".", |(d, _)| d).to_string();
            while parent != "." {
                dirs.insert(parent.clone());
                parent = parent.rsplit_once('/').map_or(".", |(d, _)| d).to_string();
            }
        }
        for dir in dirs {
            let key = project_path(&case.meta.id, &dir).to_lowercase();
            index.insert(key, (case.meta.id.clone(), dir));
        }
    }
    index
}

fn run_engine(cases: &[Case]) -> (EngineRun, HashMap<String, String>) {
    let t = std::time::Instant::now();
    let db = crate::test_utils::test_db();
    for case in cases {
        ingest_case(&db, case);
    }
    eprintln!("conformance: ingest {:.1}s", t.elapsed().as_secs_f64());
    let canon = load_pinned_advisories(&db);
    eprintln!("conformance: advisories {:.1}s", t.elapsed().as_secs_f64());
    let index = path_index(cases);
    let mut run = EngineRun::default();
    for row in db.get_all_dependency_instances().expect("instances") {
        if let Some((case, dir)) = locate(&index, &row.project_path) {
            let eco = osv_ecosystem(&row.ecosystem);
            let key = inv_key(&eco, &dir, &row.package_name, &row.version);
            run.inventory.entry(case).or_default().insert(key);
        }
    }
    let (matches, _not_compiled) = super::matching::get_matched_advisories_with_not_compiled(&db)
        .expect("conformance: matcher");
    eprintln!("conformance: matched {:.1}s", t.elapsed().as_secs_f64());
    for m in &matches {
        let eco = osv_ecosystem(&m.ecosystem);
        let group = canon
            .get(&m.advisory_id)
            .cloned()
            .unwrap_or_else(|| m.advisory_id.clone());
        for inst in &m.dependency_instances {
            let (Some((case, dir)), Some(version)) = (
                locate(&index, &inst.project_path),
                inst.installed_version.as_deref(),
            ) else {
                run.unconfirmed += 1;
                continue;
            };
            if !inst.is_version_confirmed {
                run.unconfirmed += 1;
                continue;
            }
            let key = format!("{}|{group}", inv_key(&eco, &dir, &m.package_name, version));
            run.findings.entry(case.clone()).or_default().insert(key);
            run.raw_findings
                .entry(case)
                .or_default()
                .push(serde_json::json!({
                    "ecosystem": eco, "dir": dir, "package": m.package_name,
                    "version": version, "id": m.advisory_id, "aliases": m.aliases,
                }));
        }
    }
    (run, canon)
}

// ============================================================================
// Scoring
// ============================================================================

#[derive(Default, Clone, Copy)]
struct Counts {
    tp: usize,
    fp: usize,
    fn_: usize,
}

impl Counts {
    /// No claims at all is vacuously precise.
    fn precision(self) -> f64 {
        if self.tp + self.fp == 0 {
            1.0
        } else {
            self.tp as f64 / (self.tp + self.fp) as f64
        }
    }
    /// Nothing to find is vacuously complete.
    fn recall(self) -> f64 {
        if self.tp + self.fn_ == 0 {
            1.0
        } else {
            self.tp as f64 / (self.tp + self.fn_) as f64
        }
    }
}

#[derive(Default)]
struct Score {
    findings: BTreeMap<String, Counts>,
    inventory: BTreeMap<String, Counts>,
    silent_drops: Vec<String>,
    diffs: Vec<String>,
}

fn eco_of(key: &str) -> String {
    key.split('|').next().unwrap_or("").to_string()
}

fn tally(
    by_eco: &mut BTreeMap<String, Counts>,
    diffs: &mut Vec<String>,
    case: &str,
    kind: &str,
    truth: &BTreeSet<String>,
    engine: &BTreeSet<String>,
) {
    for k in truth.union(engine) {
        let c = by_eco.entry(eco_of(k)).or_default();
        match (truth.contains(k), engine.contains(k)) {
            (true, true) => c.tp += 1,
            (true, false) => {
                c.fn_ += 1;
                diffs.push(format!("{case}\t{kind}\tMISSED\t{k}"));
            }
            (false, true) => {
                c.fp += 1;
                diffs.push(format!("{case}\t{kind}\tEXTRA\t{k}"));
            }
            (false, false) => {}
        }
    }
}

fn expected_finding_keys(case: &Case, canon: &HashMap<String, String>) -> BTreeSet<String> {
    case.expected
        .findings
        .iter()
        .map(|f| {
            let first = f.ids.first().map(String::as_str).unwrap_or_default();
            let group = canon
                .get(first)
                .cloned()
                .unwrap_or_else(|| first.to_string());
            format!(
                "{}|{group}",
                inv_key(&f.ecosystem, &f.dir, &f.package, &f.version)
            )
        })
        .collect()
}

fn score(cases: &[Case], run: &EngineRun, canon: &HashMap<String, String>) -> Score {
    let mut s = Score::default();
    let empty = BTreeSet::new();
    for case in cases {
        let id = &case.meta.id;
        let truth_inv: BTreeSet<String> =
            case.expected.inventory.packages.iter().cloned().collect();
        let engine_inv = run.inventory.get(id).unwrap_or(&empty);
        tally(
            &mut s.inventory,
            &mut s.diffs,
            id,
            "inventory",
            &truth_inv,
            engine_inv,
        );
        let truth_f = expected_finding_keys(case, canon);
        let engine_f = run.findings.get(id).unwrap_or(&empty);
        tally(
            &mut s.findings,
            &mut s.diffs,
            id,
            "finding",
            &truth_f,
            engine_f,
        );
        for lf in case
            .expected
            .lockfiles
            .iter()
            .filter(|l| l.status == "supported")
        {
            let dir = lf.dir.as_str();
            let prefix = format!("{}|{dir}|", lf.ecosystem);
            if lf.truth_packages > 0 && !engine_inv.iter().any(|k| k.starts_with(&prefix)) {
                s.silent_drops
                    .push(format!("{id}/{} ({})", lf.path, lf.format));
            }
        }
    }
    s
}

fn print_report(s: &Score, run: &EngineRun) {
    eprintln!("\n=== Dependency-engine conformance (Rust app engine) ===");
    eprintln!(
        "{:<11} {:>6} {:>5} {:>5} {:>8} {:>8} | {:>7} {:>6} {:>6} {:>8} {:>8}",
        "ecosystem", "f.TP", "FP", "FN", "prec", "recall", "inv.TP", "FP", "FN", "prec", "recall"
    );
    let ecos: BTreeSet<&String> = s.findings.keys().chain(s.inventory.keys()).collect();
    for eco in ecos {
        let f = s.findings.get(eco).copied().unwrap_or_default();
        let i = s.inventory.get(eco).copied().unwrap_or_default();
        eprintln!(
            "{:<11} {:>6} {:>5} {:>5} {:>8.4} {:>8.4} | {:>7} {:>6} {:>6} {:>8.4} {:>8.4}",
            eco,
            f.tp,
            f.fp,
            f.fn_,
            f.precision(),
            f.recall(),
            i.tp,
            i.fp,
            i.fn_,
            i.precision(),
            i.recall()
        );
    }
    eprintln!(
        "unconfirmed (version-less) matches, not scored: {}",
        run.unconfirmed
    );
    eprintln!("silent drops ({}):", s.silent_drops.len());
    for d in &s.silent_drops {
        eprintln!("  {d}");
    }
    let shown = 60.min(s.diffs.len());
    eprintln!(
        "diff ({} lines, first {shown}; full list via FOURDA_CONFORMANCE_DUMP):",
        s.diffs.len()
    );
    let is_finding = |d: &&String| d.contains("\tfinding\t");
    let findings_first = s
        .diffs
        .iter()
        .filter(is_finding)
        .chain(s.diffs.iter().filter(|d| !is_finding(d)));
    for d in findings_first.take(shown) {
        eprintln!("  {d}");
    }
}

/// `FOURDA_CONFORMANCE_DUMP=<dir>`: per-case engine output + the full diff.
fn dump(run: &EngineRun, s: &Score) {
    let Some(dir) = std::env::var_os("FOURDA_CONFORMANCE_DUMP") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("dump dir");
    let cases: BTreeSet<&String> = run.inventory.keys().chain(run.findings.keys()).collect();
    for case in cases {
        let body = serde_json::json!({
            "inventory": run.inventory.get(case).map(|s| s.iter().collect::<Vec<_>>()).unwrap_or_default(),
            "findings": run.raw_findings.get(case).cloned().unwrap_or_default(),
        });
        std::fs::write(dir.join(format!("{case}.json")), body.to_string()).expect("dump case");
    }
    std::fs::write(dir.join("diff.tsv"), s.diffs.join("\n")).expect("dump diff");
}

// ============================================================================
// Tests
// ============================================================================

/// Every lockfile-like file in the case is declared, and every declared one exists.
fn check_lockfiles_declared(case: &Case, formats: &mut BTreeSet<String>) {
    let id = &case.meta.id;
    let declared: BTreeMap<&str, &LockfileRow> = case
        .expected
        .lockfiles
        .iter()
        .map(|l| (l.path.as_str(), l))
        .collect();
    for rel in walk_files(&case.dir)
        .into_iter()
        .filter(|r| is_lockfile_like(r))
    {
        let row = declared.get(rel.as_str()).unwrap_or_else(|| {
            panic!("{id}/{rel} is a lockfile the truth does not declare (supported or unsupported)")
        });
        match row.status.as_str() {
            "supported" => {
                formats.insert(row.format.clone());
            }
            "unsupported" => assert!(
                row.reason.as_deref().is_some_and(|r| !r.is_empty()),
                "{id}/{rel}: reason"
            ),
            other => panic!("{id}/{rel}: unknown status {other}"),
        }
        assert!(!row.ecosystem.is_empty(), "{id}/{rel}: ecosystem");
    }
    for row in &case.expected.lockfiles {
        assert!(
            case.dir.join(&row.path).exists(),
            "{id}: declared {} is missing",
            row.path
        );
    }
}

/// The corpus itself: big enough, every lockfile declared (read or
/// explicitly unsupported — never silently skipped), every case traceable.
#[test]
fn corpus_is_complete_and_every_lockfile_is_declared() {
    let cases = load_cases();
    assert!(
        cases.len() >= 30,
        "G1 needs >= 30 cases, corpus has {}",
        cases.len()
    );
    let mut formats = BTreeSet::new();
    for case in &cases {
        let id = &case.meta.id;
        assert_eq!(
            &case.expected.case, id,
            "expected/{id}.json names another case"
        );
        assert_eq!(case.expected.schema, 1, "{id}: unknown expected schema");
        assert!(
            !case.meta.licence.is_empty() && !case.meta.formats.is_empty(),
            "{id}: licence/formats"
        );
        if case.meta.source == "repository" {
            assert!(
                case.meta.repo.is_some() && case.meta.commit.is_some(),
                "{id}: repo + commit"
            );
        }
        assert_eq!(
            case.expected.inventory.count,
            case.expected.inventory.packages.len(),
            "{id}: count"
        );
        check_lockfiles_declared(case, &mut formats);
    }
    assert!(
        formats.len() >= 8,
        "G1 needs >= 8 lockfile formats, corpus has {formats:?}"
    );
}

/// The ratchet: per-ecosystem precision/recall of findings and inventory,
/// and the silent-drop count, may never fall below thresholds.json.
#[test]
fn engine_meets_the_conformance_ratchet() {
    let started = std::time::Instant::now();
    let cases = load_cases();
    let (run, canon) = run_engine(&cases);
    let s = score(&cases, &run, &canon);
    print_report(&s, &run);
    dump(&run, &s);
    let thresholds: Thresholds = read_json(&corpus_dir().join("thresholds.json"));
    let mut failures = Vec::new();
    for (eco, t) in &thresholds.ecosystems {
        let f = s.findings.get(eco).copied().unwrap_or_default();
        let i = s.inventory.get(eco).copied().unwrap_or_default();
        let measured = [
            ("findings_precision", f.precision(), t.findings_precision),
            ("findings_recall", f.recall(), t.findings_recall),
            ("inventory_precision", i.precision(), t.inventory_precision),
            ("inventory_recall", i.recall(), t.inventory_recall),
        ];
        for (name, got, floor) in measured {
            if got + 1e-9 < floor {
                failures.push(format!("{eco} {name}: {got:.4} < ratchet {floor:.4}"));
            } else if got > floor + 0.0005 {
                eprintln!("ratchet can rise: {eco} {name} {floor:.4} -> {got:.4}");
            }
        }
    }
    if s.silent_drops.len() > thresholds.max_silent_drops {
        failures.push(format!(
            "silent drops {} > ratchet {}",
            s.silent_drops.len(),
            thresholds.max_silent_drops
        ));
    }
    eprintln!("conformance run: {:.1}s", started.elapsed().as_secs_f64());
    assert!(
        failures.is_empty(),
        "conformance ratchet regressed:\n{}",
        failures.join("\n")
    );
}

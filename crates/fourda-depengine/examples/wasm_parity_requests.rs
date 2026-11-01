// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The native half of the wasm parity run (single-engine C0.6).
//!
//! Turns every shared rule vector the JSON API can answer, and every
//! conformance corpus case, into a `depengine/0` request, answers it with
//! the NATIVE engine (`api::call`), and writes one JSON line per request:
//! `{"kind", "id", "request", "response", "expect"?}`. The Node harness
//! (`scripts/check-wasm-parity.cjs`) sends each request through the wasm
//! module and requires its response to equal `response` byte for byte (as
//! parsed JSON), and, where a line carries `expect` (the vector's own truth),
//! that both answers agree with it.
//!
//! What the API answers, and so what is replayed:
//! - `version_order.jsonl` -> `version_cmp`;
//! - `osv_range.jsonl` -> `affected` (the pinned record stored as the mirror
//!   stores it);
//! - `fix_target.jsonl` -> `fix_target` (every line but the `held` ones,
//!   which run the app's registry pipeline);
//! - `range_check.jsonl` -> `range_check` (the pinned records verbatim);
//! - every corpus case -> `read_lockfiles` for each directory that holds a
//!   dependency file, then one `match_inventory` over the inventory those
//!   reads give and the pinned OSV records naming its packages.
//!
//! The rest (`refresh_verdict`, `fix_command`, `holding`, `severity`,
//! `parent_release`) have no op on the provisional surface yet; the app's
//! gate runs them natively.
//!
//! Usage: `cargo run --locked --example wasm_parity_requests -- <out.jsonl>`.
//! An example, not the library: it reads files, which the library never does.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use fourda_depengine::api::call;
use fourda_depengine::ecosystem::normalize;
use fourda_depengine::lockfile::{read_dir, LockfileOutcome, MemFs};
use fourda_depengine::matching::package_key;
use fourda_depengine::osv::{stored_records, AdvisoryRecord, OsvRecord};
use serde_json::{json, Value};

type Lines = Vec<Value>;

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tests/conformance")
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn vectors(file: &str) -> Result<Lines, String> {
    let path = conformance_dir()
        .join("vectors")
        .join(format!("{file}.jsonl"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{file}.jsonl: {e}")))
        .collect()
}

/// One pinned OSV record, verbatim, and as the mirror stores it.
struct Pinned {
    verbatim: Value,
    rows: Vec<AdvisoryRecord>,
}

#[derive(Default)]
struct Records(BTreeMap<String, Pinned>);

impl Records {
    fn get(&mut self, id: &str) -> Result<&Pinned, String> {
        if !self.0.contains_key(id) {
            let path = conformance_dir().join("osv").join(format!("{id}.json"));
            let text = std::fs::read_to_string(&path).map_err(|e| format!("record {id}: {e}"))?;
            let verbatim: Value =
                serde_json::from_str(&text).map_err(|e| format!("record {id}: {e}"))?;
            let record: OsvRecord = serde_json::from_value(verbatim.clone())
                .map_err(|e| format!("record {id}: {e}"))?;
            let rows = stored_records(&record);
            self.0.insert(
                id.to_string(),
                Pinned {
                    verbatim: trimmed(&verbatim),
                    rows,
                },
            );
        }
        self.0.get(id).ok_or_else(|| format!("record {id}"))
    }

    /// The stored row of `id` for one package.
    fn row(&mut self, id: &str, eco: &str, name: &str) -> Result<AdvisoryRecord, String> {
        let key = package_key(name, eco);
        self.get(id)?
            .rows
            .iter()
            .find(|r| {
                normalize(&r.ecosystem) == normalize(eco)
                    && package_key(&r.package_name, &r.ecosystem) == key
            })
            .cloned()
            .ok_or_else(|| format!("record {id} names no {eco} {name}"))
    }
}

/// The fields of a verbatim record the engine reads (`OsvRecord`), so the
/// requests stay small.
fn trimmed(record: &Value) -> Value {
    let keep = [
        "id",
        "aliases",
        "summary",
        "database_specific",
        "affected",
        "withdrawn",
    ];
    let map = record
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| keep.contains(&k.as_str()));
    Value::Object(map.map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// A stored ranges text as the JSON the API takes.
fn ranges_value(row: &AdvisoryRecord) -> Value {
    row.affected_ranges
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Value::Null)
}

/// Answer `request` natively and record the line.
fn line(kind: &str, id: &str, request: Value, expect: Option<Value>) -> Value {
    let response: Value = serde_json::from_str(&call(&request.to_string())).unwrap_or(Value::Null);
    let mut out = json!({ "kind": kind, "id": id });
    out["request"] = request;
    out["response"] = response;
    if let Some(expect) = expect {
        out["expect"] = expect;
    }
    out
}

fn version_order_lines(out: &mut Lines) -> Result<(), String> {
    for v in vectors("version_order")? {
        let request =
            json!({ "op": "version_cmp", "eco": s(&v, "eco"), "a": s(&v, "a"), "b": s(&v, "b") });
        out.push(line(
            "version_order",
            s(&v, "id"),
            request,
            Some(json!(s(&v, "cmp"))),
        ));
    }
    Ok(())
}

fn osv_range_lines(out: &mut Lines, records: &mut Records) -> Result<(), String> {
    for v in vectors("osv_range")? {
        let row = records.row(s(&v, "record_id"), s(&v, "eco"), s(&v, "name"))?;
        let request = json!({ "op": "affected", "eco": s(&v, "eco"), "version": s(&v, "version"),
                              "ranges": ranges_value(&row) });
        let affected = v.get("affected").and_then(Value::as_bool).unwrap_or(false);
        let expect = json!({ "affected": affected, "confirmed": true });
        out.push(line("osv_range", s(&v, "id"), request, Some(expect)));
    }
    Ok(())
}

fn fix_target_lines(out: &mut Lines, records: &mut Records) -> Result<(), String> {
    for v in vectors("fix_target")?
        .into_iter()
        .filter(|v| v.get("held").is_none())
    {
        let (eco, name) = (s(&v, "eco"), s(&v, "name"));
        let (mut ranges, mut kinds) = (Vec::new(), Vec::new());
        for id in strings(&v, "record_ids") {
            let row = records.row(&id, eco, name)?;
            ranges.push(ranges_value(&row));
            kinds.push(json!(row.kind()));
        }
        let request = json!({ "op": "fix_target", "eco": eco, "installed": s(&v, "installed"),
            "ranges": ranges, "kinds": kinds,
            "registry": { "published": strings(&v, "published"), "excluded": strings(&v, "excluded") } });
        out.push(line("fix_target", s(&v, "id"), request, None));
    }
    Ok(())
}

fn range_check_lines(out: &mut Lines, records: &mut Records) -> Result<(), String> {
    for v in vectors("range_check")? {
        let mut osv = Vec::new();
        for id in strings(&v, "record_ids") {
            osv.push(records.get(&id)?.verbatim.clone());
        }
        let request = json!({ "op": "range_check", "eco": s(&v, "eco"), "name": s(&v, "name"),
            "constraint": s(&v, "constraint"), "from": v.get("from").cloned().unwrap_or(Value::Null),
            "registry": { "published": strings(&v, "releases") }, "osv": osv });
        out.push(line(
            "range_check",
            s(&v, "id"),
            request,
            Some(v["expected"].clone()),
        ));
    }
    Ok(())
}

/// Every file of a case, keyed by its host path (`/<case>/<relative>`).
fn case_files(root: &Path, case: &str) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if rel == "case.json" {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                files.insert(format!("/{case}/{rel}"), text);
            }
        }
    }
    Ok(files)
}

/// The directory part of a host path.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// The files a read of `dir` can reach: those in `dir` or above it (a `-r`
/// include, a `go.sum` beside its `go.mod`) and in `dir/requirements/`.
fn reachable<'a>(files: &'a BTreeMap<String, String>, dir: &str) -> BTreeMap<&'a str, &'a str> {
    let requirements = format!("{dir}/requirements");
    files
        .iter()
        .filter(|(p, _)| {
            let d = parent(p);
            dir == d || dir.starts_with(&format!("{d}/")) || d == requirements
        })
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect()
}

/// The inventory one directory's reads give, as the app's lockfile processor
/// records it: every installed copy (OSV ecosystem), then one resolved row
/// per package (the reader's language, the last version wins).
fn inventory_rows(dir: &str, outcomes: &[LockfileOutcome], inventory: &mut Vec<Value>) {
    let mut by_language: BTreeMap<&str, BTreeSet<(String, String)>> = BTreeMap::new();
    for outcome in outcomes {
        if let LockfileOutcome::Read(read) = outcome {
            let rows = by_language.entry(read.format.ecosystem()).or_default();
            rows.extend(
                read.packages
                    .iter()
                    .map(|p| (p.name.clone(), p.version.clone())),
            );
        }
    }
    for (language, packages) in by_language {
        let row = |name: &str, version: &str, ecosystem: &str, primary: bool| json!({ "ecosystem": ecosystem, "project": dir, "name": name, "version": version, "primary": primary });
        let mut resolved: BTreeMap<&str, &str> = BTreeMap::new();
        for (name, version) in &packages {
            inventory.push(row(name, version, normalize(language), false));
            resolved.insert(name, version);
        }
        for (name, version) in resolved {
            inventory.push(row(name, version, language, true));
        }
    }
}

/// Each pinned record's id, with the (OSV ecosystem, package key) of every
/// package it names.
type RecordIndex = Vec<(String, BTreeSet<(String, String)>)>;

/// The [`RecordIndex`] of every pinned record.
fn record_index(records: &mut Records) -> Result<RecordIndex, String> {
    let dir = conformance_dir().join("osv");
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_string_lossy()
                .strip_suffix(".json")
                .map(str::to_string)
        })
        .collect();
    ids.sort();
    let mut index = Vec::new();
    for id in ids {
        let names = records
            .get(&id)?
            .rows
            .iter()
            .map(|r| {
                (
                    normalize(&r.ecosystem).to_string(),
                    package_key(&r.package_name, &r.ecosystem),
                )
            })
            .collect();
        index.push((id, names));
    }
    Ok(index)
}

fn corpus_lines(out: &mut Lines, records: &mut Records) -> Result<(), String> {
    let index = record_index(records)?;
    let cases_dir = conformance_dir().join("cases");
    let mut cases: Vec<String> = std::fs::read_dir(&cases_dir)
        .map_err(|e| format!("{}: {e}", cases_dir.display()))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    cases.sort();
    for case in cases {
        let files = case_files(&cases_dir.join(&case), &case)?;
        let mut dirs: BTreeSet<String> = files.keys().map(|p| parent(p).to_string()).collect();
        dirs.insert(format!("/{case}"));
        let mut inventory = Vec::new();
        for dir in dirs {
            let reach = reachable(&files, &dir);
            let mut fs = MemFs::new();
            for (path, text) in &reach {
                fs.insert(path, text);
            }
            let outcomes = read_dir(&fs, &dir);
            if outcomes.is_empty() {
                continue;
            }
            inventory_rows(&dir, &outcomes, &mut inventory);
            let request = json!({ "op": "read_lockfiles", "dir": dir, "files": reach });
            out.push(line(
                "read_lockfiles",
                &format!("{case}:{dir}"),
                request,
                None,
            ));
        }
        let wanted: BTreeSet<(String, String)> = inventory
            .iter()
            .map(|p| {
                (
                    normalize(s(p, "ecosystem")).to_string(),
                    package_key(s(p, "name"), s(p, "ecosystem")),
                )
            })
            .collect();
        let mut osv = Vec::new();
        for (id, names) in &index {
            if !names.is_disjoint(&wanted) {
                osv.push(records.get(id)?.verbatim.clone());
            }
        }
        let request = json!({ "op": "match_inventory", "inventory": inventory, "osv": osv });
        out.push(line("match_inventory", &case, request, None));
    }
    Ok(())
}

fn run(path: &str) -> Result<usize, String> {
    let mut records = Records::default();
    let mut lines = Lines::new();
    version_order_lines(&mut lines)?;
    osv_range_lines(&mut lines, &mut records)?;
    fix_target_lines(&mut lines, &mut records)?;
    range_check_lines(&mut lines, &mut records)?;
    corpus_lines(&mut lines, &mut records)?;
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut w = std::io::BufWriter::new(file);
    for l in &lines {
        writeln!(w, "{l}").map_err(|e| format!("{path}: {e}"))?;
    }
    w.flush().map_err(|e| format!("{path}: {e}"))?;
    Ok(lines.len())
}

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: wasm_parity_requests <out.jsonl>");
        std::process::exit(2);
    };
    match run(&path) {
        Ok(n) => println!("wrote {n} native request/response lines to {path}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The `range_check` rule vectors (rc-001..) against this crate alone, so
//! the crate's own test run (and CI's `Depengine` job) proves the constraint
//! check without the app. Each line names its package's pinned OSV records
//! (`src-tauri/tests/conformance/osv/`); they are stored exactly as the
//! mirror stores them (`osv::stored_records`) and the package's rows are
//! handed to `range_check` with the line's releases. The authoritative gate,
//! with the shrink-only skip baseline, stays in the app
//! (`src-tauri/src/osv/vectors_tests.rs`), which calls the same function.

use std::path::PathBuf;

use fourda_depengine::osv::{stored_records, OsvRecord};
use fourda_depengine::range_check::{package_advisories, range_check, RangeQuery};
use fourda_depengine::registry::RegistryReleases;
use serde_json::{json, Value};

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

/// The engine's answer for one vector line, in the vector's `expected` shape.
fn answer(v: &Value) -> Result<Value, String> {
    let (eco, name) = (s(v, "eco"), s(v, "name"));
    let mut rows = Vec::new();
    for id in strings(v, "record_ids") {
        let path = conformance_dir().join("osv").join(format!("{id}.json"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("record {id} not pinned: {e}"))?;
        let record: OsvRecord =
            serde_json::from_str(&text).map_err(|e| format!("record {id}: {e}"))?;
        rows.extend(stored_records(&record));
    }
    let advisories = package_advisories(&rows, eco, name);
    if advisories.is_empty() {
        return Err(format!("no pinned record names {eco} {name}"));
    }
    let registry = RegistryReleases {
        published: strings(v, "releases"),
        excluded: Vec::new(),
    };
    let got = range_check(&RangeQuery {
        ecosystem: eco,
        constraint: s(v, "constraint"),
        from: v.get("from").and_then(Value::as_str),
        registry: &registry,
        advisories: &advisories,
        held_unsafe: &[],
    })
    .map_err(|e| e.to_string())?;
    Ok(json!({
        "releases_in_range": got.releases_in_range,
        "vulnerable_in_range": got.vulnerable_in_range,
        "smallest_safe_in_range": got.smallest_safe_in_range,
        "newest_safe_in_range": got.newest_safe_in_range,
        "smallest_safe_upgrade": got.smallest_safe_upgrade.flatten(),
    }))
}

#[test]
fn range_check_vectors_hold() {
    let dir = conformance_dir().join("vectors");
    let skips: Value = std::fs::read_to_string(dir.join("engines_skip.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let text = std::fs::read_to_string(dir.join("range_check.jsonl"))
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    let (mut pass, mut problems) = (0usize, Vec::new());
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).unwrap_or_default();
        let id = s(&v, "id");
        let outcome = answer(&v).and_then(|got| {
            if got == v["expected"] {
                Ok(())
            } else {
                Err(format!("got {got}, want {}", v["expected"]))
            }
        });
        match (outcome, skips["rust"].get(id).is_some()) {
            (Ok(()), false) => pass += 1,
            (Err(_), true) => {}
            (Ok(()), true) => problems.push(format!("{id} passes but is in the rust skip list")),
            (Err(why), false) => problems.push(format!("{id}: {why}")),
        }
    }
    println!("range_check vectors (fourda-depengine): {pass} pass");
    assert!(pass > 0, "no range_check vectors ran");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

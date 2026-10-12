// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The `version_order` rule vectors against this crate alone, so the crate's
//! own test run (and CI's `Depengine` job) proves the ordering without the
//! app. The authoritative gate over all six vector files, with the
//! shrink-only skip baseline, stays in the app
//! (`src-tauri/src/osv/vectors_tests.rs`), which calls this crate's functions;
//! the `osv_range` and `fix_target` files need the app's OSV-record merge
//! (`osv::sync::merge_affected`), which moves here in C0.2.

use std::cmp::Ordering;
use std::path::PathBuf;

use fourda_depengine::version_order::OrderedVersion;
use serde_json::Value;

fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/tests/conformance/vectors")
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

#[test]
fn version_order_vectors_hold() {
    let dir = vectors_dir();
    let skips: Value = std::fs::read_to_string(dir.join("engines_skip.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let text = std::fs::read_to_string(dir.join("version_order.jsonl"))
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    let (mut pass, mut problems) = (0usize, Vec::new());
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).unwrap_or_default();
        let id = s(&v, "id");
        let want = match s(&v, "cmp") {
            "<" => Some(Ordering::Less),
            "=" => Some(Ordering::Equal),
            ">" => Some(Ordering::Greater),
            _ => None,
        };
        let eco = s(&v, "eco");
        let got = OrderedVersion::parse_in(s(&v, "a"), eco)
            .zip(OrderedVersion::parse_in(s(&v, "b"), eco))
            .and_then(|(x, y)| x.compare(&y));
        let skipped = skips["rust"].get(id).is_some();
        match (want.is_some() && got == want, skipped) {
            (true, false) => pass += 1,
            (false, true) => {}
            (true, true) => problems.push(format!("{id} passes but is in the rust skip list")),
            (false, false) => problems.push(format!("{id}: got {got:?}, want {want:?}")),
        }
    }
    println!("version_order vectors (fourda-depengine): {pass} pass");
    assert!(pass > 0, "no version_order vectors ran");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

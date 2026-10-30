// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `tests/fixtures/parent_links_case.json` is one `parent_links` call (C0.4)
//! played through its `needs` round-trip: the first `request`, the files a
//! host would find (`disk`), and every round's request and answer. This test
//! replays the rounds natively, answering each round's needs from `disk`, and
//! requires the recorded rounds; `scripts/check-wasm.cjs` replays the same
//! requests through the wasm module, so wasm == native on every round.
//! `FOURDA_DEPENGINE_FIXTURE=1` rewrites `rounds` from the native engine.

use serde_json::{json, Map, Value};

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("parent_links_case.json")
}

/// The answer `disk` gives one need, under the `provided` key it goes in.
fn answer(disk: &Value, need: &Value) -> (&'static str, String, Value) {
    let field = |k: &str| need[k].as_str().unwrap_or_default().to_string();
    let (section, key) = match need["kind"].as_str().unwrap_or_default() {
        "manifest" => ("manifests", field("path")),
        "listing" => ("listings", field("dir")),
        "crate_manifest" => (
            "crate_manifests",
            format!("{}@{}", field("name"), field("version")),
        ),
        _ => ("npm_locks", field("dir")),
    };
    let found = disk[section].get(&key).cloned().unwrap_or(Value::Null);
    (section, key, found)
}

/// Every round, natively: the request, then the answer.
fn play(first: &Value, disk: &Value) -> Vec<Value> {
    let mut request = first.clone();
    let mut rounds = Vec::new();
    for round in 0..=fourda_depengine::needs::MAX_ROUNDS {
        request["round"] = json!(round);
        let response: Value =
            serde_json::from_str(&fourda_depengine::api::call(&request.to_string()))
                .unwrap_or_default();
        rounds.push(json!({ "request": request.clone(), "response": response.clone() }));
        let Some(needs) = response["result"]["needs"].as_array() else {
            break;
        };
        let provided: Map<String, Value> = request["provided"]
            .as_object_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        let mut provided = Value::Object(provided);
        for need in needs {
            let (section, key, found) = answer(disk, need);
            if provided.get(section).is_none() {
                provided[section] = json!({});
            }
            provided[section][key] = found;
        }
        request["provided"] = provided;
    }
    rounds
}

#[test]
fn the_native_engine_plays_the_parent_links_rounds_as_recorded() {
    let path = fixture_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut fixture: Value = serde_json::from_str(&text).unwrap_or_default();
    assert_eq!(
        fixture["request"]["op"],
        "parent_links",
        "{}",
        path.display()
    );
    let rounds = play(&fixture["request"], &fixture["disk"]);
    let last = &rounds[rounds.len() - 1]["response"];
    assert_eq!(last["ok"], true, "{last}");
    assert!(
        last["result"]["links"]
            .as_array()
            .is_some_and(|l| !l.is_empty()),
        "the case ends with links: {last}"
    );
    if std::env::var_os("FOURDA_DEPENGINE_FIXTURE").is_some() {
        fixture["rounds"] = Value::Array(rounds);
        let mut out = serde_json::to_string_pretty(&fixture).unwrap_or_default();
        out.push('\n');
        std::fs::write(&path, out).unwrap_or_default();
        return;
    }
    assert!(
        Value::Array(rounds) == fixture["rounds"],
        "{} is stale: rerun with FOURDA_DEPENGINE_FIXTURE=1",
        path.display()
    );
}

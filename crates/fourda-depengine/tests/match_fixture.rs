// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `tests/fixtures/match_inventory_case.json` is one real conformance case
//! (`src-tauri/tests/conformance/cases/py-fastapi-requirements`) as a
//! `match_inventory` call: the inventory the app's lockfile readers produce,
//! the pinned OSV records naming its packages, and the engine's answer. The
//! app's `osv::conformance_tests` holds the request and answer to the readers
//! and the engine; `scripts/check-wasm.cjs` replays it through the wasm
//! module. This test replays it natively, so all three agree.

use serde_json::Value;

#[test]
fn the_native_engine_answers_the_fixture_case_as_recorded() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("match_inventory_case.json");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let fixture: Value = serde_json::from_str(&text).unwrap_or_default();
    let request = &fixture["request"];
    assert_eq!(request["op"], "match_inventory", "{}", path.display());
    let got: Value = serde_json::from_str(&fourda_depengine::api::call(&request.to_string()))
        .unwrap_or_default();
    assert_eq!(got["ok"], true, "{got}");
    assert!(
        got["result"]["findings"]
            .as_array()
            .is_some_and(|f| !f.is_empty()),
        "the fixture case has findings"
    );
    assert!(
        got == fixture["response"],
        "native answer differs from the fixture"
    );
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The `depengine/0` JSON surface, call by call (`api::call`).

use super::*;

fn result(request: &str) -> Value {
    let v: Value = serde_json::from_str(&call(request)).unwrap_or_default();
    assert_eq!(v["schema"], SCHEMA);
    assert_eq!(v["ok"], true, "{v}");
    v["result"].clone()
}

#[test]
fn version_cmp_reads_pypi_as_pep440() {
    let r = result(r#"{"op":"version_cmp","eco":"PyPI","a":"1.0.0-1","b":"1.0.0"}"#);
    assert_eq!(r, json!(">"));
    let r = result(r#"{"op":"version_cmp","eco":"npm","a":"x","b":"1.0.0"}"#);
    assert_eq!(r, Value::Null);
}

#[test]
fn affected_and_fix_target_answer_from_the_rules() {
    let ranges = r#"[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.4"}]}]"#;
    let r = result(&format!(
        r#"{{"op":"affected","eco":"npm","version":"1.2.3","ranges":{ranges}}}"#
    ));
    assert_eq!(r, json!({"affected": true, "confirmed": true}));
    let r = result(&format!(
        r#"{{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[{ranges}]}}"#
    ));
    assert_eq!(
        r,
        json!({"line_fixes": ["1.2.4"], "walk": {"version": "1.2.4", "clears_all": true}, "withheld": []})
    );
    // The registry withdrew 1.2.4: the next release it offers is the fix.
    let r = result(&format!(
        r#"{{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[{ranges}],
            "registry":{{"published":["1.2.3","1.2.4","1.2.5"],"excluded":["1.2.4"]}}}}"#
    ));
    assert_eq!(
        r,
        json!({"line_fixes": ["1.2.5"], "walk": {"version": "1.2.5", "clears_all": true}, "withheld": ["1.2.4"]})
    );
}

#[test]
fn match_inventory_reads_registry_facts_per_package() {
    let r = result(
        r#"{"op":"match_inventory",
            "inventory":[{"ecosystem":"rust","project":"/p","name":"h2","version":"0.3.16","primary":true}],
            "osv":[{"id":"RUSTSEC-2023-0034","affected":[{"package":{"name":"h2","ecosystem":"crates.io"},
              "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"0.3.17"}]}]}]}],
            "registry":[{"ecosystem":"crates.io","name":"H2","published":["0.3.16","0.3.17","0.3.18"],"excluded":["0.3.17"]}]}"#,
    );
    let copy = &r["findings"][0]["dependency_instances"][0];
    assert_eq!(copy["target_version"], "0.3.18");
    assert_eq!(copy["withheld"], json!(["0.3.17"]));
}

#[test]
fn match_inventory_stores_verbatim_records_and_matches_them() {
    let r = result(
        r#"{"op":"match_inventory",
            "inventory":[{"ecosystem":"javascript","project":"/p","name":"ws","version":"7.5.9","primary":true}],
            "osv":[{"id":"GHSA-ws","modified":"x","affected":[{"package":{"name":"ws","ecosystem":"npm"},
              "ranges":[{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.10"}]}]}]},
                   {"id":"GHSA-ws","affected":[{"package":{"name":"ws","ecosystem":"npm"}}]}]}"#,
    );
    assert_eq!(
        r["records"], 1,
        "a repeated (id, package, ecosystem) row is stored once"
    );
    let f = &r["findings"][0];
    assert_eq!(f["advisory_id"], "GHSA-ws");
    assert_eq!(f["is_version_confirmed"], true);
    assert_eq!(f["fixed_version"], "7.5.10");
    assert_eq!(f["dependency_instances"][0]["clean_version"], "7.5.10");
    assert_eq!(f["project_paths"], json!(["/p"]));
}

#[test]
fn copies_without_a_primary_row_are_an_error_not_an_all_clear() {
    let request = r#"{"op":"match_inventory",
        "inventory":[{"ecosystem":"npm","project":"/p","name":"ws","version":"7.5.9","primary":false}],
        "osv":[{"id":"GHSA-ws","affected":[{"package":{"name":"ws","ecosystem":"npm"},
          "ranges":[{"type":"SEMVER","events":[{"introduced":"0"}]}]}]}]}"#;
    let v: Value = serde_json::from_str(&call(request)).unwrap_or_default();
    assert_eq!(v["ok"], false, "{v}");
    assert!(v["error"]
        .as_str()
        .is_some_and(|e| e.contains("no primary")));
    // `primary` is required: leaving it out is a parse error too.
    let v: Value = serde_json::from_str(&call(
        r#"{"op":"match_inventory","inventory":[{"ecosystem":"npm","project":"/p","name":"ws","version":"7.5.9"}]}"#,
    ))
    .unwrap_or_default();
    assert_eq!(v["ok"], false, "{v}");
    // An empty inventory is a truthful empty answer.
    let r = result(r#"{"op":"match_inventory","inventory":[]}"#);
    assert_eq!(r["findings"], json!([]));
}

#[test]
fn read_lockfiles_reads_the_files_given() {
    let r = result(
        r#"{"op":"read_lockfiles","dir":"/p","files":{
            "/p/requirements.txt":"-r base.txt\nflask==3.0.0\n",
            "/p/base.txt":"click==8.1.7\n",
            "/p/pom.xml":"<project/>"}}"#,
    );
    let outcomes = r["outcomes"].as_array().cloned().unwrap_or_default();
    // base.txt is not a requirements file by name; it is read as the include.
    assert_eq!(outcomes.len(), 2, "{r}");
    let req = &outcomes[0]["read"];
    assert_eq!(req["path"], "/p/requirements.txt");
    assert_eq!(req["format"], "requirements");
    assert_eq!(req["packages"][0]["name"], "click", "the include is read");
    assert_eq!(req["packages"][1]["name"], "flask");
    assert_eq!(outcomes[1]["unsupported"]["path"], "/p/pom.xml");
}

#[test]
fn read_lockfiles_over_its_caps_is_an_error() {
    let many: serde_json::Map<String, Value> = (0..=MAX_LOCKFILE_FILES)
        .map(|i| (format!("/p/{i}.txt"), json!("")))
        .collect();
    let v: Value = serde_json::from_str(&call(
        &json!({"op": "read_lockfiles", "dir": "/p", "files": many}).to_string(),
    ))
    .unwrap_or_default();
    assert_eq!(v["ok"], false, "{}", v["error"]);
    assert!(v["error"].as_str().is_some_and(|e| e.contains("files")));

    let big = "x".repeat(MAX_LOCKFILE_BYTES);
    let v: Value = serde_json::from_str(&call(
        &json!({"op": "read_lockfiles", "dir": "/p",
                "files": {"/p/requirements.txt": big}})
        .to_string(),
    ))
    .unwrap_or_default();
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().is_some_and(|e| e.contains("bytes")));
}

#[test]
fn range_check_answers_from_the_records_given() {
    let osv = r#"[{"id":"GHSA-ws","affected":[{"package":{"name":"ws","ecosystem":"npm"},
        "ranges":[{"type":"SEMVER","events":[{"introduced":"7.0.0"},{"fixed":"7.5.10"}]}]}]}]"#;
    let r = result(&format!(
        r#"{{"op":"range_check","eco":"npm","name":"WS","constraint":"7.x","from":"7.5.9",
            "registry":{{"published":["7.5.9","7.5.10","7.5.11","8.0.0"]}},"osv":{osv}}}"#
    ));
    assert_eq!(r["releases_in_range"], 3);
    assert_eq!(r["vulnerable_in_range"], 1);
    assert_eq!(r["smallest_safe_in_range"], "7.5.10");
    assert_eq!(r["newest_safe_in_range"], "7.5.11");
    assert_eq!(r["smallest_safe_upgrade"], "7.5.10");
    // No advisories stated at all is an error; an empty list is "none".
    let bare = r#"{"op":"range_check","eco":"npm","name":"ws","constraint":"7.x"}"#;
    let v: Value = serde_json::from_str(&call(bare)).unwrap_or_default();
    assert_eq!(v["ok"], false, "{v}");
    let r = result(
        r#"{"op":"range_check","eco":"npm","name":"ws","constraint":"7.x","advisories":[],
            "registry":{"published":["7.0.0"]}}"#,
    );
    assert_eq!(r["smallest_safe_in_range"], "7.0.0");
    let v: Value = serde_json::from_str(&call(
        r#"{"op":"range_check","eco":"npm","name":"ws","constraint":"latest","advisories":[]}"#,
    ))
    .unwrap_or_default();
    assert!(
        v["error"]
            .as_str()
            .is_some_and(|e| e.contains("constraint")),
        "{v}"
    );
}

#[test]
fn a_bad_request_is_an_error_not_a_panic() {
    let v: Value = serde_json::from_str(&call("{\"op\":\"nope\"}")).unwrap_or_default();
    assert_eq!(v["ok"], false);
    let v: Value = serde_json::from_str(&call("not json")).unwrap_or_default();
    assert_eq!(v["ok"], false);
}

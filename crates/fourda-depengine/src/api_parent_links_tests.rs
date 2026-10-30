// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The `parent_links` op's contract (C0.4): what it refuses, what it never
//! cites, and that it ends the round-trip itself, naming what went unanswered.

use super::*;

fn respond(request: &Value) -> Value {
    serde_json::from_str(&call(&request.to_string())).unwrap_or_default()
}

/// The fixture's first request (`tests/fixtures/parent_links_case.json`).
fn fixture_request() -> Value {
    let fixture: Value =
        serde_json::from_str(include_str!("../tests/fixtures/parent_links_case.json"))
            .unwrap_or_default();
    fixture["request"].clone()
}

fn braces_ranges() -> Value {
    json!([[{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "3.0.3"}]}]])
}

#[test]
fn a_request_without_ranges_or_a_round_is_refused() {
    let request = fixture_request();
    assert_eq!(
        respond(&request)["ok"],
        true,
        "the fixture request is valid"
    );

    let mut no_ranges = request.clone();
    if let Some(o) = no_ranges.as_object_mut() {
        o.remove("ranges");
    }
    let v = respond(&no_ranges);
    assert_eq!(v["ok"], false, "{v}");
    assert!(
        v["error"].as_str().is_some_and(|e| e.contains("ranges")),
        "{v}"
    );

    // An empty list is refused too: nothing could be checked as clear.
    let mut empty = request.clone();
    empty["ranges"] = json!([]);
    let v = respond(&empty);
    assert_eq!(v["ok"], false, "{v}");
    assert!(
        v["error"].as_str().is_some_and(|e| e.contains("ranges")),
        "{v}"
    );

    let mut no_round = request;
    if let Some(o) = no_round.as_object_mut() {
        o.remove("round");
    }
    let v = respond(&no_round);
    assert_eq!(v["ok"], false, "{v}");
    assert!(
        v["error"].as_str().is_some_and(|e| e.contains("round")),
        "{v}"
    );
}

/// A newer micromatch elsewhere that still resolves the vulnerable braces
/// 3.0.2 is no resolution; one that resolves the fixed 3.0.3 is.
#[test]
fn a_newer_parent_still_on_the_vulnerable_copy_is_not_cited() {
    let request = json!({
        "op": "parent_links", "eco": "npm", "package": "braces", "round": MAX_ROUNDS,
        "lines": [{"installed_version": "3.0.2", "target_version": "3.0.3",
                   "sites": [{"project_path": "/w/taxonomy", "is_direct": false}]}],
        "edges": [
            {"project": "/w/taxonomy", "parent": "micromatch", "parent_version": "4.0.5", "child_spec": "3.0.2"},
            {"project": "/w/new", "parent": "micromatch", "parent_version": "4.0.8", "child_spec": "3.0.2"},
            {"project": "/w/clean", "parent": "micromatch", "parent_version": "4.1.0", "child_spec": "3.0.3"}
        ],
        "installed": [
            {"project": "/w/taxonomy", "package": "braces", "version": "3.0.2", "is_direct": false},
            {"project": "/w/taxonomy", "package": "micromatch", "version": "4.0.5", "is_direct": true},
            {"project": "/w/new", "package": "braces", "version": "3.0.2", "is_direct": false},
            {"project": "/w/new", "package": "micromatch", "version": "4.0.8", "is_direct": true},
            {"project": "/w/clean", "package": "braces", "version": "3.0.3", "is_direct": false},
            {"project": "/w/clean", "package": "micromatch", "version": "4.1.0", "is_direct": true}
        ],
        "ranges": braces_ranges(),
        "managers": {"/w/taxonomy": "pnpm"},
        "provided": {}
    });
    let v = respond(&request);
    assert_eq!(v["ok"], true, "{v}");
    let links = &v["result"]["links"];
    assert_eq!(links.as_array().map_or(0, Vec::len), 1, "{v}");
    assert_eq!(
        links[0]["resolutions"],
        json!([{"parent_version": "4.1.0", "child_version": "3.0.3", "project": "/w/clean"}])
    );
}

/// The probe of the C0.4 review: a host that stops before answering
/// anything. The npm app's tree went unanswered, so no parent is named there
/// from a range (anymatch holds 3.0.3, audit E8): the copy takes the semver
/// route, and the answer names every need nobody answered.
#[test]
fn an_exhausted_round_trip_names_what_was_never_answered() {
    let mut early = fixture_request();
    early["provided"] = json!({ "exhausted": true });
    let mut capped = fixture_request();
    capped["round"] = json!(MAX_ROUNDS);
    for request in [early, capped] {
        let v = respond(&request);
        assert_eq!(v["ok"], true, "{v}");
        let r = &v["result"];
        assert!(r.get("needs").is_none(), "a final answer: {r}");
        let links = r["links"].as_array().cloned().unwrap_or_default();
        assert!(
            links.iter().all(|l| l["project"] != "/w/app"),
            "no parent named from a range alone: {r}"
        );
        let unanswered = r["unanswered"].as_array().cloned().unwrap_or_default();
        assert!(
            unanswered.contains(&json!({"kind": "npm_lock", "dir": "/w/app"})),
            "{r}"
        );
        let app = r["unlinked"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .find(|u| u["project"] == "/w/app");
        assert_eq!(
            app.map(|u| u["refresh"].clone()),
            Some(json!({"enough": "semver"})),
            "{r}"
        );
    }
}

/// A host that answers under its own key (a lowercased crate name) never
/// answers the need. The engine ends the loop at the cap anyway, and names
/// the need it never got.
#[test]
fn a_host_answering_under_the_wrong_key_still_gets_a_final_answer() {
    let mut request = json!({
        "op": "parent_links", "eco": "crates.io", "package": "mio", "round": 0,
        "lines": [{"installed_version": "0.8.0", "target_version": "0.8.11",
                   "sites": [{"project_path": "/p", "is_direct": false}]}],
        "edges": [{"project": "/p", "parent": "Tokio", "parent_version": "1.0.0", "child_spec": "0.8.0"}],
        "installed": [
            {"project": "/p", "package": "tokio", "version": "1.0.0", "is_direct": true},
            {"project": "/p", "package": "mio", "version": "0.8.0", "is_direct": false}
        ],
        "ranges": [[{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "0.8.11"}]}]],
        "managers": {"/p": "cargo"},
        "provided": {}
    });
    let mut calls = 0;
    let result = loop {
        calls += 1;
        let v = respond(&request);
        assert_eq!(v["ok"], true, "{v}");
        let r = v["result"].clone();
        if r.get("needs").is_none() || calls > 10 {
            break r;
        }
        request["provided"] = json!({
            "crate_manifests": {"tokio@1.0.0": "[dependencies]\nmio = \"0.8\"\n"},
            "npm_locks": {"/p": null}
        });
        request["round"] = json!(calls);
    };
    assert!(calls <= MAX_ROUNDS + 1, "{calls} calls");
    assert!(result.get("needs").is_none(), "{result}");
    assert_eq!(
        result["unanswered"],
        json!([{"kind": "crate_manifest", "name": "Tokio", "version": "1.0.0"}])
    );
    // The requirement never came: the verdict is the semver inference.
    assert_eq!(
        result["links"][0]["refresh"]["enough"], "semver",
        "{result}"
    );
}

#[test]
fn provided_listings_count_toward_the_byte_cap() {
    let mut request = fixture_request();
    request["provided"] = json!({
        "listings": {"/w/taxonomy/node_modules/.pnpm": ["x".repeat(MAX_LOCKFILE_BYTES)]}
    });
    let v = respond(&request);
    assert_eq!(v["ok"], false, "{}", v["error"]);
    assert!(v["error"].as_str().is_some_and(|e| e.contains("bytes")));
}

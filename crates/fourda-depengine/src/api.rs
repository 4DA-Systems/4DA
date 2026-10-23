// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The provisional JSON call surface, one string in and one string out. The
//! wasm module exports it (`wasm.rs`); it also keeps every rule reachable from
//! the module, so the module's size in CI measures the real engine.
//!
//! Provisional on purpose: the schema is `depengine/0` and nothing outside
//! this repository calls it. The versioned `depengine/1` contract
//! (single-engine design §3.2: `scan`, `fix_paths`, `range_check`,
//! `version_cmp`) replaces it once the engine owns inventory (C0.2+).
//!
//! Requests (`ranges` is an OSV `ranges` array as the app stores it, with
//! enumerated `versions` as an `ENUMERATED` range):
//! - `{"op":"version_cmp","eco":"PyPI","a":"1.0.post1","b":"1.0"}` → `"<" | "=" | ">" | null`
//! - `{"op":"affected","eco":"npm","version":"1.2.3","ranges":[...]}` → `{"affected":bool,"confirmed":bool}`
//! - `{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[[...],[...]],
//!   "registry":{"published":[..],"excluded":[..]}}` (`registry` optional: the
//!   package's releases and the ones its registry withdrew, `RegistryReleases`)
//!   → `{"line_fixes":[..],"walk":{"version":"..","clears_all":bool}|null,"withheld":[..]}`
//! - `{"op":"match_inventory","inventory":[Package..],"osv":[verbatim OSV records],
//!   "records":[AdvisoryRecord..]}` → `{"records":n,"findings":[{"advisory_id",
//!   "package_name","ecosystem","installed_version","fixed_version",
//!   "is_version_confirmed","project_paths","dependency_instances":[..]}]}`.
//!   `osv` records are stored the way one store pass of the app's mirror
//!   stores them ([`crate::osv::stored_records`]; within this request the
//!   first row per (id, package, ecosystem) wins, after any `records` given
//!   as already-stored rows). Across syncs the app's table is an upsert
//!   (`ON CONFLICT DO UPDATE`), so there the LAST write of a row wins; give
//!   the current rows as `records` to reproduce a mirror state.
//!   Every `inventory` row must say `primary`; copies with no primary row
//!   are an error, not an empty result. Optional `registry`:
//!   `[{"ecosystem","name","published":[..],"excluded":[..]}]`.

use std::cmp::Ordering;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::ecosystem::normalize as normalize_ecosystem;
use crate::fix_target::{installable_line_fix, walk_target_kinds, AdvisoryKind};
use crate::matching::{inventory_problem, match_inventory_with, package_key, Package};
use crate::osv::{stored_records, AdvisoryRecord, OsvRecord};
use crate::range::check_version_affected;
use crate::registry::{RegistryIndex, RegistryReleases};
use crate::version_order::OrderedVersion;

/// The schema every response carries.
pub const SCHEMA: &str = "depengine/0";

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    VersionCmp {
        eco: String,
        a: String,
        b: String,
    },
    Affected {
        eco: String,
        version: Option<String>,
        ranges: Option<Value>,
    },
    FixTarget {
        eco: String,
        installed: String,
        ranges: Vec<Option<Value>>,
        #[serde(default)]
        registry: RegistryReleases,
        /// Parallel to `ranges`: malware / maintenance-notice records.
        #[serde(default)]
        kinds: Vec<AdvisoryKind>,
    },
    MatchInventory {
        inventory: Vec<Package>,
        #[serde(default)]
        records: Vec<AdvisoryRecord>,
        #[serde(default)]
        osv: Vec<OsvRecord>,
        #[serde(default)]
        registry: Vec<PackageReleases>,
    },
}

/// One package's registry facts in a `match_inventory` request.
#[derive(Deserialize)]
struct PackageReleases {
    ecosystem: String,
    name: String,
    #[serde(flatten)]
    releases: RegistryReleases,
}

fn registry_index(rows: Vec<PackageReleases>) -> RegistryIndex {
    rows.into_iter()
        .map(|r| {
            let key = (
                package_key(&r.name, &r.ecosystem),
                normalize_ecosystem(&r.ecosystem).to_string(),
            );
            (key, r.releases)
        })
        .collect()
}

/// The `match_inventory` op: stored rows plus verbatim records, matched. An
/// inventory with copies but no primary rows is an error, never an empty
/// all-clear ([`inventory_problem`]).
fn match_op(
    inventory: &[Package],
    mut records: Vec<AdvisoryRecord>,
    osv: &[OsvRecord],
    registry: &RegistryIndex,
) -> Result<Value, String> {
    if let Some(problem) = inventory_problem(inventory) {
        return Err(problem);
    }
    let key = |r: &AdvisoryRecord| {
        (
            r.advisory_id.clone(),
            r.package_name.clone(),
            r.ecosystem.clone(),
        )
    };
    let mut seen: std::collections::HashSet<(String, String, String)> =
        records.iter().map(key).collect();
    for row in osv.iter().flat_map(stored_records) {
        if seen.insert(key(&row)) {
            records.push(row);
        }
    }
    let findings: Vec<Value> = match_inventory_with(inventory, &records, registry, |_, _, _| {})
        .into_iter()
        .map(|f| {
            let r = &records[f.record];
            json!({
                "advisory_id": r.advisory_id,
                "package_name": r.package_name,
                "ecosystem": r.ecosystem,
                "installed_version": f.installed_version,
                "fixed_version": f.fixed_version,
                "is_version_confirmed": f.is_version_confirmed,
                "project_paths": f.project_paths,
                "dependency_instances": f.dependency_instances,
            })
        })
        .collect();
    Ok(json!({ "records": records.len(), "findings": findings }))
}

/// A ranges array as the stored JSON text the rules read.
fn stored(ranges: Option<&Value>) -> Option<String> {
    ranges.filter(|v| !v.is_null()).map(Value::to_string)
}

fn answer(request: Request) -> Result<Value, String> {
    Ok(match request {
        Request::VersionCmp { eco, a, b } => {
            let order = OrderedVersion::parse_in(&a, &eco)
                .zip(OrderedVersion::parse_in(&b, &eco))
                .and_then(|(x, y)| x.compare(&y));
            match order {
                Some(Ordering::Less) => json!("<"),
                Some(Ordering::Equal) => json!("="),
                Some(Ordering::Greater) => json!(">"),
                None => Value::Null,
            }
        }
        Request::Affected {
            eco,
            version,
            ranges,
        } => {
            let (affected, confirmed) =
                check_version_affected(&eco, version.as_deref(), &stored(ranges.as_ref()));
            json!({ "affected": affected, "confirmed": confirmed })
        }
        Request::FixTarget {
            eco,
            installed,
            ranges,
            registry,
            kinds,
        } => {
            let stored: Vec<Option<String>> = ranges.iter().map(|r| stored(r.as_ref())).collect();
            let line_fixes: Vec<String> = stored
                .iter()
                .filter_map(|r| installable_line_fix(&eco, &installed, r, &registry).fix)
                .collect();
            let refs: Vec<&Option<String>> = stored.iter().collect();
            let outcome = walk_target_kinds(&eco, &installed, &refs, &kinds, &registry);
            let walk = outcome
                .target
                .map(|w| json!({ "version": w.version, "clears_all": w.clears_all }));
            let mut out =
                json!({ "line_fixes": line_fixes, "walk": walk, "withheld": outcome.withheld });
            // Only when set, so every earlier answer keeps its exact shape.
            if !outcome.introduces.is_empty() {
                out["introduces"] = json!(outcome.introduces);
            }
            if !outcome.malware.is_empty() {
                out["malware"] = json!({ "from": outcome.malware_from, "indices": outcome.malware });
            }
            out
        }
        Request::MatchInventory {
            inventory,
            records,
            osv,
            registry,
        } => match_op(&inventory, records, &osv, &registry_index(registry))?,
    })
}

/// Answer one JSON request. Never panics on bad input: a request that does
/// not parse, or one the engine cannot answer truthfully, comes back as
/// `{"ok": false, "error": ...}`.
pub fn call(request: &str) -> String {
    let error = |e: String| json!({ "schema": SCHEMA, "ok": false, "error": e });
    let response = match serde_json::from_str::<Request>(request) {
        Ok(req) => match answer(req) {
            Ok(result) => json!({ "schema": SCHEMA, "ok": true, "result": result }),
            Err(e) => error(e),
        },
        Err(e) => error(e.to_string()),
    };
    response.to_string()
}

#[cfg(test)]
mod tests {
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
    fn a_bad_request_is_an_error_not_a_panic() {
        let v: Value = serde_json::from_str(&call("{\"op\":\"nope\"}")).unwrap_or_default();
        assert_eq!(v["ok"], false);
        let v: Value = serde_json::from_str(&call("not json")).unwrap_or_default();
        assert_eq!(v["ok"], false);
    }
}

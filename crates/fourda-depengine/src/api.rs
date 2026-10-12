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
//! - `{"op":"fix_target","eco":"npm","installed":"1.2.3","ranges":[[...],[...]]}`
//!   → `{"line_fixes":[..],"walk":{"version":"..","clears_all":bool}|null}`

use std::cmp::Ordering;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::fix_target::{fix_for_version, walk_target};
use crate::range::check_version_affected;
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
    },
}

/// A ranges array as the stored JSON text the rules read.
fn stored(ranges: Option<&Value>) -> Option<String> {
    ranges.filter(|v| !v.is_null()).map(Value::to_string)
}

fn answer(request: Request) -> Value {
    match request {
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
        } => {
            let stored: Vec<Option<String>> = ranges.iter().map(|r| stored(r.as_ref())).collect();
            let line_fixes: Vec<String> = stored
                .iter()
                .filter_map(|r| fix_for_version(&eco, &installed, r))
                .collect();
            let refs: Vec<&Option<String>> = stored.iter().collect();
            let walk = walk_target(&eco, &installed, &refs)
                .map(|w| json!({ "version": w.version, "clears_all": w.clears_all }));
            json!({ "line_fixes": line_fixes, "walk": walk })
        }
    }
}

/// Answer one JSON request. Never panics on bad input: a request that does
/// not parse comes back as `{"ok": false, "error": ...}`.
pub fn call(request: &str) -> String {
    let response = match serde_json::from_str::<Request>(request) {
        Ok(req) => json!({ "schema": SCHEMA, "ok": true, "result": answer(req) }),
        Err(e) => json!({ "schema": SCHEMA, "ok": false, "error": e.to_string() }),
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
            json!({"line_fixes": ["1.2.4"], "walk": {"version": "1.2.4", "clears_all": true}})
        );
    }

    #[test]
    fn a_bad_request_is_an_error_not_a_panic() {
        let v: Value = serde_json::from_str(&call("{\"op\":\"nope\"}")).unwrap_or_default();
        assert_eq!(v["ok"], false);
        let v: Value = serde_json::from_str(&call("not json")).unwrap_or_default();
        assert_eq!(v["ok"], false);
    }
}

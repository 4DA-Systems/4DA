// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `parent_release` lines with `mode: "graph"` (review of #981, S5): a whole
//! installed graph, the advisories, and the registry's RAW abbreviated
//! documents (deprecation notices kept, nothing pre-filtered). The runner
//! stores them as the app would (`release_facts` parses the documents) and
//! runs the app's own pipeline end to end: the matcher's targets, the raised
//! line targets, `parent_hint::load_routes` (the loader, the parent's own row
//! target, the declared levers and their hops), and `raised_row_target`.
//! Commands are not checked here (the project has no manifest on disk; the
//! `decide` lines check them through `direct_fix`).

use serde_json::{json, Value};

use crate::db::Database;

use super::parent_hint::{load_routes, ParentLink, ParentStanding};

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn arr<'a>(v: &'a Value, key: &str) -> Vec<&'a Value> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect()
}

/// An exact pin spelled as the range it is: with no lockfile on disk the app
/// reads a range-shaped edge as a requirement, not an exact one.
fn as_range(req: &str) -> String {
    if semver::Version::parse(req.trim()).is_ok() {
        format!(">={0} <={0}", req.trim())
    } else {
        req.to_string()
    }
}

fn store_advisories(db: &Database, advisories: &[&Value]) {
    for a in advisories {
        let ranges: Vec<Value> = arr(a, "ranges")
            .into_iter()
            .map(|w| {
                let mut events = vec![json!({"introduced": s(w, "introduced")})];
                for bound in ["fixed", "last_affected"] {
                    if let Some(b) = w.get(bound).and_then(Value::as_str) {
                        events.push(json!({ bound: b }));
                    }
                }
                json!({"type": "SEMVER", "events": events})
            })
            .collect();
        let _ = db.upsert_osv_advisory(
            s(a, "id"),
            s(a, "id"),
            None,
            s(a, "package"),
            "npm",
            Some(&Value::Array(ranges).to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        // Optional `aliases` / `cwe_ids`, stored as a sync stores them.
        let list = |key: &str| {
            let items: Vec<&str> = arr(a, key).into_iter().filter_map(Value::as_str).collect();
            (!items.is_empty()).then(|| json!(items).to_string())
        };
        if let Some(aliases) = list("aliases") {
            let _ = db.conn.lock().execute(
                "UPDATE osv_advisories SET aliases = ?2 WHERE advisory_id = ?1",
                rusqlite::params![s(a, "id"), aliases],
            );
        }
        let _ = db.set_osv_advisory_cwe_ids(
            s(a, "id"),
            s(a, "package"),
            "npm",
            list("cwe_ids").as_deref(),
        );
    }
}

fn build(v: &Value, project: &str) -> Database {
    let nodes: Vec<(String, String, bool)> = arr(v, "nodes")
        .into_iter()
        .map(|n| {
            (
                n[0].as_str().unwrap_or_default().to_string(),
                n[1].as_str().unwrap_or_default().to_string(),
                n[2].as_bool().unwrap_or(false),
            )
        })
        .collect();
    let edges: Vec<(String, String, String, String)> = arr(v, "edges")
        .into_iter()
        .map(|e| {
            let t = |i: usize| e[i].as_str().unwrap_or_default().to_string();
            (t(0), t(1), t(2), as_range(&t(3)))
        })
        .collect();
    let node_refs: Vec<_> = nodes
        .iter()
        .map(|(n, v, d)| (n.as_str(), v.as_str(), *d))
        .collect();
    let edge_refs: Vec<_> = edges
        .iter()
        .map(|(p, pv, c, r)| (p.as_str(), pv.as_str(), c.as_str(), r.as_str()))
        .collect();
    let db = super::parent_release_fixture::graph(project, &node_refs, &edge_refs, &[]);
    store_advisories(&db, &arr(v, "advisories"));
    let docs: Vec<(String, Value)> = v
        .get("registry")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, d)| (k.clone(), d.clone()))
        .collect();
    super::release_facts::store_docs_for_tests(&db, &docs);
    db
}

/// What the vectors compare for one link.
fn link_json(link: &ParentLink) -> Value {
    let release = |st: &ParentStanding| match st {
        ParentStanding::Admits(r) => {
            json!({"to": r.to, "smallest": r.smallest, "floor": r.floor, "brings": r.brings})
        }
        ParentStanding::Malicious { .. } => json!("malicious"),
        ParentStanding::Undecided { .. } => json!("undecided"),
        ParentStanding::NoneAdmits { .. } => json!("none_admits"),
        ParentStanding::Deprecated => json!("deprecated"),
        ParentStanding::Newest { .. } => json!("latest"),
        _ => json!("unread"),
    };
    let levers: Vec<Value> = link
        .through
        .iter()
        .flat_map(|t| t.all())
        .map(|l| json!({"name": l.name, "release": release(&l.standing)}))
        .collect();
    json!({
        "parent": link.parent,
        "release": release(&link.standing),
        "levers": levers,
        "lever_unknown": link.lever_unknown.is_some(),
    })
}

fn check_link(db: &Database, want: &Value) -> Result<(), String> {
    let pkg = s(want, "pkg");
    let matches = super::matching::get_matched_advisories(db).map_err(|e| e.to_string())?;
    let group: Vec<_> = matches.iter().filter(|m| m.package_name == pkg).collect();
    let projects: Vec<String> = group
        .iter()
        .flat_map(|m| {
            m.dependency_instances
                .iter()
                .map(|d| d.project_path.clone())
        })
        .collect();
    let lines = super::raised_lines::raised_line_targets(db, &group, &projects).lines;
    let line = lines
        .iter()
        .find(|l| l.installed_version == s(want, "installed"))
        .ok_or_else(|| format!("{pkg}: no line at {}", s(want, "installed")))?;
    if line.target_version.as_deref() != Some(s(want, "target")) {
        return Err(format!(
            "{pkg}: target {:?}, want {}",
            line.target_version,
            s(want, "target")
        ));
    }
    let routes = load_routes(db, "npm", pkg, &lines);
    for w in arr(want, "links") {
        let got = routes
            .links
            .iter()
            .find(|l| l.parent == s(w, "parent"))
            .map(link_json)
            .ok_or_else(|| format!("{pkg}: no link through {}", s(w, "parent")))?;
        if &got != w {
            return Err(format!("{pkg}: got {got}, want {w}"));
        }
    }
    Ok(())
}

pub(super) fn check(v: &Value) -> Result<(), String> {
    let project = format!("d:/vectors/{}", s(v, "id"));
    let db = build(v, &project);
    let expect = v.get("expect").ok_or("no expect")?;
    for want in arr(expect, "copies") {
        check_link(&db, want)?;
    }
    for row in arr(expect, "rows") {
        let got = super::parent_release_load::raised_row_target(
            &db,
            "npm",
            &project,
            s(row, "pkg"),
            s(row, "installed"),
            s(row, "own_target"),
        )
        .map(|(to, _)| to);
        let want = row.get("to").and_then(Value::as_str);
        if got.as_deref() != want {
            return Err(format!("{} row: got {got:?}, want {want:?}", s(row, "pkg")));
        }
    }
    Ok(())
}

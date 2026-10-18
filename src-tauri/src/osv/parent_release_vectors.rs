// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The `parent_release` rule vectors (audit H1), run by `vectors_tests`
//! against the real functions: `parent_release::decide` for the releases,
//! `direct_upgrade::direct_fix` on a project built from the vector for the
//! command (the declared operator kept), and `Manager::refresh_command` for
//! the refresh that follows an install that keeps the vulnerable copy.

use serde_json::{json, Value};

use super::direct_upgrade::{direct_fix, DirectFix};
use super::fix_path::{Dialect, Manager};
use super::parent_release::{
    decide, is_major, Advisories, Advisory, Child, ReleaseReq, Releases, RequirementHistory, RowIn,
    Standing,
};

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn opt(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// `[{id, ranges: [{introduced, fixed | last_affected}]}]` as stored ranges
/// JSON, one OSV SEMVER range per window.
fn advisories(v: Option<&Value>) -> Vec<Advisory> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|a| {
            let ranges: Vec<Value> = a
                .get("ranges")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|w| {
                    let mut events = vec![json!({"introduced": s(w, "introduced")})];
                    for bound in ["fixed", "last_affected"] {
                        if let Some(b) = opt(w, bound) {
                            events.push(json!({ bound: b }));
                        }
                    }
                    json!({"type": "SEMVER", "events": events})
                })
                .collect();
            Advisory::new(
                s(a, "id"),
                &strings(a.get("aliases")),
                &strings(a.get("cwe_ids")),
                &opt(a, "summary").unwrap_or_default(),
                Some(Value::Array(ranges).to_string()),
            )
        })
        .collect()
}

/// `{release: "req" | null | {"peer": "req"}}`.
fn history(v: Option<&Value>) -> RequirementHistory {
    v.and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(version, r)| {
            let req = match r {
                Value::String(t) => ReleaseReq {
                    reqs: vec![t.clone()],
                    peer: false,
                },
                Value::Object(o) => ReleaseReq {
                    reqs: o
                        .get("peer")
                        .and_then(Value::as_str)
                        .map(|p| vec![p.to_string()])
                        .unwrap_or_default(),
                    peer: true,
                },
                _ => ReleaseReq::default(),
            };
            (version.clone(), req)
        })
        .collect()
}

struct Row {
    pkg: String,
    installed: String,
    target: String,
    releases: Vec<String>,
    advisories: Vec<Advisory>,
    history: RequirementHistory,
}

fn outcome(standing: &Standing) -> &'static str {
    match standing {
        Standing::Release(_) => "release",
        Standing::Deprecated => "deprecated",
        Standing::Latest => "latest",
        Standing::NoneAdmits { .. } => "none_admits",
        Standing::Unread => "unread",
        Standing::Undecided(_) => "undecided",
        Standing::Malicious(_) => "malicious",
    }
}

/// The command that moves a declared npm dependency, read by `direct_fix`
/// from a project holding the vector's declared spec.
fn command(
    tool: &str,
    name: &str,
    declared: Option<&str>,
    installed: &str,
    to: &str,
) -> Result<Option<String>, String> {
    let Some(declared) = declared else {
        return Ok(None);
    };
    if tool != "npm" {
        return Err(format!("no project shape for tool {tool}"));
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let pkg_json = json!({ "dependencies": { name: declared } });
    std::fs::write(dir.path().join("package.json"), pkg_json.to_string())
        .map_err(|e| e.to_string())?;
    std::fs::write(
        dir.path().join("package-lock.json"),
        "{\"lockfileVersion\": 3}\n",
    )
    .map_err(|e| e.to_string())?;
    // The command the lever names: the in-range refresh when the declaration
    // already admits the release (pr-001), else the install that keeps the
    // operator; exhaustive so a new kind of fix cannot drop out silently.
    Ok(match direct_fix(dir.path(), "npm", name, installed, to) {
        Some(DirectFix::Command(c) | DirectFix::Refresh(c)) => Some(c),
        Some(DirectFix::Instruction(_)) | None => None,
    })
}

pub(super) fn check_parent_release(v: &Value) -> Result<(), String> {
    if s(v, "mode") == "graph" {
        return super::parent_release_graph_vectors::check(v);
    }
    let eco = s(v, "eco");
    let tool = s(v, "tool");
    let p = v.get("parent").ok_or("no parent")?;
    let parent_name = s(p, "name");
    let parent_installed = s(p, "installed");
    let parent = Releases {
        versions: strings(p.get("releases")),
        latest: opt(p, "latest"),
        deprecated: p
            .get("deprecated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let parent_advisories = Advisories {
        ecosystem: eco,
        list: advisories(p.get("advisories")),
    };
    let rows: Vec<Row> = v
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|r| Row {
            pkg: s(r, "pkg").to_string(),
            installed: s(r, "installed").to_string(),
            target: s(r, "target").to_string(),
            releases: strings(r.get("releases")),
            advisories: advisories(r.get("advisories")),
            history: history(r.get("requires")),
        })
        .collect();
    let ads: Vec<Advisories<'_>> = rows
        .iter()
        .map(|r| Advisories {
            ecosystem: eco,
            list: r.advisories.clone(),
        })
        .collect();
    let newest: Vec<Option<String>> = rows
        .iter()
        .zip(&ads)
        .map(|(r, a)| {
            a.newest(&Releases {
                versions: r.releases.clone(),
                ..Releases::default()
            })
        })
        .collect();
    let cleans: Vec<Box<dyn Fn(&str) -> bool + '_>> = ads
        .iter()
        .zip(&newest)
        .map(|(a, n)| {
            Box::new(move |x: &str| a.usable(x, n.as_deref())) as Box<dyn Fn(&str) -> bool>
        })
        .collect();
    let inputs: Vec<RowIn<'_>> = rows
        .iter()
        .zip(&cleans)
        .map(|(r, clean)| RowIn {
            child: Child {
                ecosystem: eco,
                installed: &r.installed,
                target: &r.target,
                releases: &r.releases,
                clean: clean.as_ref(),
            },
            history: Some(&r.history),
            unknown: None,
        })
        .collect();
    let own_target = p.get("own_target").and_then(Value::as_str);
    let decision = decide(
        &parent,
        parent_installed,
        own_target,
        &parent_advisories,
        &inputs,
        &[],
        Dialect::Npm,
    );

    let expect = v.get("expect").ok_or("no expect")?;
    let want_target = opt(expect, "parent_target");
    if decision.row_target != want_target {
        return Err(format!(
            "parent_target: got {:?}, want {want_target:?}",
            decision.row_target
        ));
    }
    let want_rows: Vec<&Value> = expect
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    if want_rows.len() != decision.rows.len() {
        return Err(format!(
            "rows: got {}, want {}",
            decision.rows.len(),
            want_rows.len()
        ));
    }
    let declared = (s(p, "lever") == "direct").then(|| s(p, "declared"));
    for ((row, out), want) in rows.iter().zip(&decision.rows).zip(want_rows) {
        let to = out.to.as_ref();
        let to_v = to.map(|f| f.version.clone());
        let got = json!({
            "pkg": row.pkg,
            "outcome": outcome(&out.standing),
            "smallest": out.smallest().map(|f| f.version.clone()),
            "to": to_v,
            "breaking": to.is_some_and(|f| is_major(parent_installed, &f.version)),
            "requirement": to.and_then(|f| f.requirement.clone()),
            "peer": to.is_some_and(|f| f.peer),
            "resolves_to": to.and_then(|f| f.resolves_to.clone()),
            "command": match to {
                Some(f) => command(tool, parent_name, declared, parent_installed, &f.version)?,
                None => None,
            },
            "refresh": to
                .filter(|f| f.keeps_installed)
                .and_then(|_| Manager::Npm.refresh_command(&row.pkg, &row.installed, &row.target)),
        });
        if &got != want {
            return Err(format!("{}: got {got}, want {want}", row.pkg));
        }
    }
    Ok(())
}

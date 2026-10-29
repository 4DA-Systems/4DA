// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `fix_target` lines with a `held` block (audit 6.2.5 D-4): a fix target is
//! judged by what it exact-pins or narrowly ranges. Each line holds the
//! installed package, every advisory (`held.advisories`, the graph lines'
//! shape), and the RAW npm abbreviated documents (`held.registry`). The
//! runner stores them as the app would and runs the app's own pipeline:
//! the matcher, the held-dependency pass (`osv::held_targets`) and the line
//! targets.
//!
//! `held.lookup`:
//! - `"ok"`: every package the registry block names was in a successful npm
//!   advisory sync (`release_facts::record_held_synced`), so a package with no
//!   advisory listed has none;
//! - `"down"`: the held packages' advisories cannot be read (only the
//!   installed package's own are stored, nothing is recorded as synced): the
//!   MCP's OSV lookup failing.
//!
//! Expected: `target` (the line target), `held_outcome` (`clean`: the rule has
//! nothing to say; `moved`, `not_clean`, `not_checked`), and when present
//! `held_from` (moved: the advisories' own target) and `searched` (not_clean:
//! the releases judged).

use serde_json::Value;

use fourda_depengine::held::HeldOutcome;

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn outcome_name(held: Option<&HeldOutcome>) -> &'static str {
    match held {
        None => "clean",
        Some(HeldOutcome::Moved { .. }) => "moved",
        Some(HeldOutcome::Blocked { .. }) => "not_clean",
        Some(HeldOutcome::Unchecked { .. }) => "not_checked",
    }
}

pub(super) fn check(v: &Value) -> Result<(), String> {
    let (name, installed) = (s(v, "name"), s(v, "installed"));
    let held = v.get("held").ok_or("no held block")?;
    let project = format!("d:/vectors/{}", s(v, "id"));
    let db = super::parent_release_fixture::graph(&project, &[(name, installed, true)], &[], &[]);
    let down = s(held, "lookup") == "down";
    let advisories: Vec<&Value> = held
        .get("advisories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|a| !down || s(a, "package") == name)
        .collect();
    super::parent_release_graph_vectors::store_advisories(&db, &advisories);
    let docs: Vec<(String, Value)> = held
        .get("registry")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, d)| (k.clone(), d.clone()))
        .collect();
    super::release_facts::store_docs_for_tests(&db, &docs);
    if !down {
        let names: Vec<String> = docs.iter().map(|(k, _)| k.clone()).collect();
        super::release_facts::record_held_synced(&db, &names);
    }

    let matches = super::matching::get_matched_advisories(&db).map_err(|e| e.to_string())?;
    let group: Vec<_> = matches.iter().filter(|m| m.package_name == name).collect();
    let projects: Vec<String> = group
        .iter()
        .flat_map(|m| {
            m.dependency_instances
                .iter()
                .map(|d| d.project_path.clone())
        })
        .collect();
    let lines = super::fix_target::line_targets(&group, &projects);
    let line = lines
        .iter()
        .find(|l| l.installed_version == installed)
        .ok_or_else(|| format!("{name}: no line at {installed}"))?;
    let want_target = v.get("target").and_then(Value::as_str);
    if line.target_version.as_deref() != want_target {
        return Err(format!(
            "target: got {:?}, want {want_target:?} ({:?})",
            line.target_version, line.held
        ));
    }
    let got = outcome_name(line.held.as_ref());
    if got != s(v, "held_outcome") {
        return Err(format!(
            "held_outcome: got {got} ({:?}), want {}",
            line.held,
            s(v, "held_outcome")
        ));
    }
    if let Some(from) = v.get("held_from").and_then(Value::as_str) {
        match &line.held {
            Some(HeldOutcome::Moved { from: f, .. }) if f == from => {}
            other => return Err(format!("held_from: got {other:?}, want {from}")),
        }
    }
    if let Some(want) = v.get("searched").and_then(Value::as_array) {
        let want: Vec<&str> = want.iter().filter_map(Value::as_str).collect();
        match &line.held {
            Some(HeldOutcome::Blocked { searched, .. }) if *searched == want => {}
            other => return Err(format!("searched: got {other:?}, want {want:?}")),
        }
    }
    Ok(())
}

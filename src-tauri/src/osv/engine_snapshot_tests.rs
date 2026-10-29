// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Before/after proof for the single-engine extraction (Stage C0): dumps every
//! output the dependency engine feeds, from a database SNAPSHOT, so a refactor
//! that must not change behaviour can be diffed byte for byte.
//!
//! Dumped: the matcher (`get_matched_advisories_with_not_compiled`), the
//! scorer's per-advisory exposure (`exposure_for_advisory`, every mirror row),
//! Preemption (`get_preemption_feed`), the Brief's facts, Signal Lane 1
//! (`get_stack_changes`) and the Upgrade Plan (items and work order). Only
//! `computed_at` (wall clock) is removed.
//!
//! Run it on a COPY (the opens migrate it and the surfaces write caches; take
//! a fresh copy per run, never the live file):
//!
//! ```text
//! FOURDA_DB_PATH=<snapshot copy> FOURDA_DATA_DIR=<scratch> \
//! FOURDA_ENGINE_SNAPSHOT_OUT=<file.json> cargo test --lib \
//!     osv::engine_snapshot_tests -- --ignored --nocapture
//! ```

use serde_json::{json, Value};

fn strip_volatile(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("computed_at");
            for child in map.values_mut() {
                strip_volatile(child);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_volatile),
        _ => {}
    }
}

#[test]
#[ignore = "requires FOURDA_DB_PATH pointing at a database snapshot copy"]
fn dump_engine_outputs_from_snapshot() {
    let (Ok(_), Ok(out)) = (
        std::env::var("FOURDA_DB_PATH"),
        std::env::var("FOURDA_ENGINE_SNAPSHOT_OUT"),
    ) else {
        return;
    };
    let db = crate::get_database().expect("open snapshot");

    let (matches, not_compiled) =
        super::matching::get_matched_advisories_with_not_compiled(db).expect("matcher");
    let advisories = db.get_all_osv_advisories().expect("advisories");
    let exposure: Vec<String> = advisories
        .iter()
        .map(|a| {
            let e = super::advisory_match::exposure_for_advisory(db, a);
            format!(
                "{}|{}|{} => {:?} unconfirmed={} checked={}",
                a.advisory_id,
                a.ecosystem,
                a.package_name,
                e.confirmed,
                e.unconfirmed,
                e.copies_checked
            )
        })
        .collect();
    let preemption = crate::preemption::get_preemption_feed().expect("preemption feed");
    let brief = crate::brief_facts::build_brief_facts(db);
    let lane1 = tauri::async_runtime::block_on(crate::evidence::stack_change::get_stack_changes(
        Some(true),
    ))
    .expect("lane 1");
    let plan = crate::evidence::build_upgrade_plan(db);

    let mut dump = json!({
        "matcher": {
            "matches": matches,
            "not_compiled": not_compiled.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>(),
        },
        "exposure": exposure,
        "preemption": preemption,
        "brief": brief,
        "lane1": lane1,
        "upgrade_plan": { "items": plan.items, "steps": plan.steps, "drops": plan.drops },
    });
    strip_volatile(&mut dump);
    let text = serde_json::to_string_pretty(&dump).expect("serialize dump");
    std::fs::write(&out, &text).expect("write dump");
    println!(
        "engine snapshot: {} matches, {} not compiled, {} exposure rows, {} bytes -> {out}",
        matches.len(),
        not_compiled.len(),
        exposure.len(),
        text.len()
    );
}

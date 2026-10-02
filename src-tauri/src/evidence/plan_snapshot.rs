// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The persisted Upgrade Plan envelope (blueprint D-1, DB-as-interface):
//! written to `kv_store` after every plan compute and read out-of-process by
//! the `4da plan` CLI and the MCP `upgrade_planner`. Split out of
//! `upgrade_plan.rs` (file-size gate) when the envelope gained the work order
//! (AD-049, schema v4).

use crate::db::Database;

use super::types::EvidenceItem;
use super::upgrade_steps::UpgradeStep;

/// `kv_store` key under which the persisted plan snapshot lives (single,
/// latest-wins). The MCP server reads this same key from the shared SQLite file.
const PLAN_KV_KEY: &str = "upgrade_plan_snapshot";

/// Persisted-shape version. Bump on an incompatible change to
/// [`super::types::UpgradePlanSnapshot`] / the item JSON; a reader that sees a
/// higher version treats the snapshot as absent (fail closed). v4: `steps`
/// (AD-049).
const PLAN_SCHEMA_VERSION: u32 = 4;

/// Persist the ranked plan to `kv_store` (blueprint D-1, DB-as-interface) so it
/// survives restart and is readable out-of-process (the `4da plan` CLI reads
/// this key; the MCP handoff reads it too). Called from BOTH the GUI feed compute
/// and the headless engine cycle — persists EVERY computed plan, including an
/// empty one, so a reader can tell "evaluated, nothing to do" (fresh
/// `generated_at`, 0 items) from "never computed" (no key). Best-effort: a write
/// error is logged, never propagated into the caller.
///
/// `steps` is the work order built with `items` ([`super::build_upgrade_plan`]); a
/// step whose item is not in `items` is never written (AD-049: every step
/// names an item the reader can see).
pub fn persist_upgrade_plan(
    db: &Database,
    items: &[EvidenceItem],
    steps: &[UpgradeStep],
    validation_drop_count: u32,
    engine_run_id: Option<i64>,
) {
    let steps: Vec<UpgradeStep> = steps
        .iter()
        .filter(|s| items.iter().any(|i| i.id == s.item_id))
        .cloned()
        .collect();
    let generated = chrono::Utc::now();
    // Freshness FLOOR of the security data: the oldest ecosystem sync timestamp
    // (lexicographic min of the fixed-width `YYYY-MM-DD HH:MM:SS` = chronological
    // oldest). A reader pairs this with `expires_at` to judge staleness honestly.
    let source_freshness = db
        .get_osv_sync_statuses()
        .ok()
        .and_then(|statuses| statuses.into_iter().filter_map(|s| s.last_synced_at).min());
    // Staleness horizon: the plan is only as fresh as the security data it read,
    // and that data is refreshed on the OSV sync cadence. State that horizon so a
    // reader judges staleness without knowing 4DA's policy.
    let expires = generated + chrono::Duration::hours(crate::osv::sync::osv_sync_max_age_hours());
    // The inventory the plan was computed from (already sorted by the query) —
    // used for both the change-detection hash and the coverage gate.
    let instances = db.get_all_dependency_instances().unwrap_or_default();
    let dependency_inventory_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        // Volatile columns (row id, detected_at) are excluded on purpose — the
        // hash tracks the identity set (project, ecosystem, package, version).
        for r in &instances {
            hasher.update(r.project_path.as_bytes());
            hasher.update([0u8]);
            hasher.update(r.ecosystem.as_bytes());
            hasher.update([0u8]);
            hasher.update(r.package_name.as_bytes());
            hasher.update([0u8]);
            hasher.update(r.version.as_bytes());
            hasher.update(*b"\n");
        }
        hex::encode(hasher.finalize())
    };
    let snapshot = super::types::UpgradePlanSnapshot {
        schema_version: PLAN_SCHEMA_VERSION,
        generated_at: generated.to_rfc3339(),
        expires_at: expires.to_rfc3339(),
        generator_version: env!("CARGO_PKG_VERSION").to_string(),
        entitlement_scope_at_generation: if crate::settings::is_signal() {
            "signal".to_string()
        } else {
            "free".to_string()
        },
        // Green iff the multi-version inventory (Phase 92) is populated — a reader
        // must not trust negative/close verdicts without it.
        multi_version_coverage: !instances.is_empty(),
        dependency_inventory_hash,
        validation_drop_count,
        source_freshness,
        engine_run_id,
        item_count: items.len(),
        items: items.to_vec(),
        steps,
    };
    match serde_json::to_string(&snapshot) {
        Ok(json) => {
            if let Err(e) = db.set_kv(PLAN_KV_KEY, &json) {
                tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to persist upgrade plan snapshot");
            }
        }
        Err(e) => {
            tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to serialize upgrade plan snapshot");
        }
    }
}

/// Read the persisted plan snapshot. Returns `None` when absent, unparseable, or
/// written by an incompatible schema version (fail closed — a reader must never
/// act on a snapshot it cannot fully trust).
///
/// Test-exercised only: the shipped `4da plan` CLI reads the `kv_store` key via
/// raw SQL (it deliberately does not link `fourda_lib`), so it does NOT call this
/// `Database`-based reader. This fn's production caller is the in-app Phase-2a
/// reader (operator-gated), still pending.
// Moved 2026-09-24 from 2026-10-01: the in-app Phase-2a reader that wires this
// is still operator-gated, and deleting a tested, fail-closed reader ahead of
// that decision would pre-empt it. Same review date as the seven markers #681
// moved. If Phase-2a is not scheduled by then, delete this fn and its tests.
#[allow(dead_code)] // REMOVE BY 2026-11-15 — wired by the in-app Phase-2a reader
pub fn read_upgrade_plan_snapshot(db: &Database) -> Option<super::types::UpgradePlanSnapshot> {
    let json = db.get_kv(PLAN_KV_KEY).ok().flatten()?;
    let snapshot: super::types::UpgradePlanSnapshot = serde_json::from_str(&json).ok()?;
    if snapshot.schema_version != PLAN_SCHEMA_VERSION {
        tracing::debug!(
            target: "4da::upgrade_plan",
            found = snapshot.schema_version,
            expected = PLAN_SCHEMA_VERSION,
            "upgrade plan snapshot schema mismatch — treated as absent"
        );
        return None;
    }
    Some(snapshot)
}

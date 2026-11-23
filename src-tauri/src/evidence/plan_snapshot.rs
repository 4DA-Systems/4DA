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
/// (AD-049). v5 (Y5, 2026-11-10): a step's `mechanism` may be
/// `lockfile_refresh`, `parent_update` or `blocked_upstream`, and
/// `lockfile_or_parent_update` narrowed to "route undecided"; a step may
/// carry `blocked_upstream` holds. A v4 reader switching on `mechanism`
/// would misread them, so it must see the bump and fail closed.
const PLAN_SCHEMA_VERSION: u32 = 5;

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
) -> Option<String> {
    let steps: Vec<UpgradeStep> = steps
        .iter()
        .filter(|s| items.iter().any(|i| i.id == s.item_id))
        .cloned()
        .collect();
    let generated = chrono::Utc::now();
    let (source_freshness, expires) = advisory_horizon(db, generated);
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
        Ok(json) => match db.set_kv(PLAN_KV_KEY, &json) {
            Ok(()) => Some(snapshot.generated_at),
            Err(e) => {
                tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to persist upgrade plan snapshot");
                None
            }
        },
        Err(e) => {
            tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to serialize upgrade plan snapshot");
            None
        }
    }
}

/// The advisory data a plan was built from, and how long the plan stays fresh.
///
/// `source_freshness` is the freshness FLOOR: the oldest verified sync time of
/// the ecosystems the inventory needs, by the rule the advisory readiness line
/// uses (`osv::readiness::classify`), so an ecosystem never verified leaves no
/// floor and a row for an ecosystem the inventory no longer holds is not one.
/// A plan cannot be fresher than that data, so it expires when the data falls
/// due for a sync ([`plan_expiry`]); a reader comparing `expires_at` with the
/// clock needs no knowledge of 4DA's sync policy.
fn advisory_horizon(
    db: &Database,
    generated: chrono::DateTime<chrono::Utc>,
) -> (Option<String>, chrono::DateTime<chrono::Utc>) {
    let max_age = chrono::Duration::hours(crate::osv::sync::osv_sync_max_age_hours());
    let required = match crate::osv::sync::auditable_coverage(db) {
        Ok(coverage) => coverage.versioned,
        // The inventory could not be read: nothing dates the plan's data.
        Err(_) => return (None, generated),
    };
    if required.is_empty() {
        // Nothing to check against advisories: no data to grow stale.
        return (None, generated + max_age);
    }
    let statuses = db.get_osv_sync_statuses().unwrap_or_default();
    let floor = crate::osv::readiness::classify(&required, &statuses, generated.naive_utc(), false)
        .synced_at;
    let expires = plan_expiry(generated, floor.as_deref(), max_age);
    (floor, expires)
}

/// `generated + max_age`, never later than `source_freshness + max_age`: a
/// plan built from advisory data older than `max_age` is already expired when
/// written, and one built with no dated advisory data (`None`, or a time that
/// does not parse) expires at `generated`.
fn plan_expiry(
    generated: chrono::DateTime<chrono::Utc>,
    source_freshness: Option<&str>,
    max_age: chrono::Duration,
) -> chrono::DateTime<chrono::Utc> {
    let Some(synced) = source_freshness
        .and_then(|at| chrono::NaiveDateTime::parse_from_str(at, "%Y-%m-%d %H:%M:%S").ok())
        .map(|at| at.and_utc())
    else {
        return generated;
    };
    (synced + max_age).min(generated + max_age)
}

/// Attribute the stored snapshot to the engine run that built it. The engine
/// builds the plan early in its cycle (dependency intelligence first, audit
/// R7), before that cycle's receipt exists; this sets `engine_run_id` once the
/// receipt is written. Compare-and-set in one statement: only when the stored
/// snapshot is still the one this cycle wrote (`generated_at` matches), so a
/// plan the app wrote in between is never credited to the engine run.
/// Returns whether the snapshot was attributed. Best-effort.
pub fn stamp_plan_engine_run(db: &Database, engine_run_id: i64, generated_at: &str) -> bool {
    let conn = db.conn.lock();
    match conn.execute(
        "UPDATE kv_store SET value = json_set(value, '$.engine_run_id', ?1)
         WHERE key = ?2 AND json_valid(value) AND json_extract(value, '$.generated_at') = ?3",
        rusqlite::params![engine_run_id, PLAN_KV_KEY, generated_at],
    ) {
        Ok(n) => n == 1,
        Err(e) => {
            tracing::warn!(target: "4da::upgrade_plan", error = %e, "failed to attribute the plan snapshot to its engine run");
            false
        }
    }
}

/// Read the persisted plan snapshot. Returns `None` when absent, unparseable, or
/// written by an incompatible schema version (fail closed — a reader must never
/// act on a snapshot it cannot fully trust).
///
/// Test-only: the reference reader the plan tests use to prove what the
/// out-of-process readers get (the `4da plan` CLI and the MCP
/// `upgrade_planner` read the `kv_store` key with their own SQL; the app reads
/// the plan live). It waited for an in-app Phase-2a reader under a dead-code
/// expiry marker dated 2026-11-15; that reader is not scheduled, so the
/// production copy is gone and only the contract tests keep it.
#[cfg(test)]
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

#[cfg(test)]
#[path = "plan_snapshot_tests.rs"]
mod tests;

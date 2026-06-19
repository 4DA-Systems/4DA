// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Pre-baked briefing snapshot — the killer feature of Sovereign Cold Boot.
//!
//! Conventional cold boot UX:
//!   power on -> wait 5-30s -> backend ready -> request briefing -> wait
//!   another 2-5s for LLM synthesis -> finally see content.
//!
//! 4DA cold boot UX with this module:
//!   power on -> 4DA window appears with YESTERDAY'S briefing in <200ms
//!   -> small "refreshing" indicator -> backend catches up silently ->
//!   new items animate in when ready.
//!
//! How it works
//!
//! 1. **Save**: Whenever a briefing is generated (steady state) AND when
//!    the app shuts down, the latest `BriefingNotification` (including the
//!    full LLM synthesis text) is serialized to JSON at
//!    `<data_dir>/briefing_snapshot.json`. Atomic write via temp+rename.
//!
//! 2. **Load**: On cold boot, BEFORE React mounts, `main.tsx` calls the
//!    privileged `get_briefing_snapshot` Tauri command. The command reads
//!    the JSON synchronously, validates the TTL (default 24h), and returns
//!    `Option<BriefingSnapshot>`. The frontend renders it as the first
//!    paint, with a freshness banner ("Brief from Mon 9:14 AM — refreshing
//!    intelligence in background").
//!
//! 3. **Expire**: If the snapshot is older than `MAX_AGE_HOURS`, it is
//!    silently ignored (still on disk for diagnostics) and the frontend
//!    falls through to its normal empty/loading state.
//!
//! ## Cross-platform safety
//!
//! - Atomic writes use `tempfile::NamedTempFile::persist`, which calls
//!   `rename()` on POSIX and `MoveFileEx` on Windows — both atomic.
//! - The snapshot file is the only authoritative source: we never truncate
//!   in place (would corrupt mid-write).
//! - File errors never propagate. A missing/corrupt snapshot just means
//!   the frontend shows its normal first-run state. The user is never
//!   shown an error from this module.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::monitoring_briefing::BriefingNotification;
use crate::state::get_db_path;

/// Snapshot file format version. Bump if breaking changes to fields below.
/// Older versions are silently ignored on load.
const SNAPSHOT_VERSION: u32 = 1;

/// How old a snapshot can be before we refuse to use it.
/// 24 hours covers: same-day reopens, overnight closures, weekend gaps.
/// Snapshots older than this would feel staler than helpful.
const MAX_AGE_SECS: u64 = 24 * 3600;

/// On-disk snapshot of the most recent briefing.
///
/// Contains the full briefing including the LLM-synthesized narrative.
/// The user-facing experience: open 4DA tomorrow morning, see yesterday's
/// "Three things to know today" paragraph instantly, then watch it refresh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingSnapshot {
    /// Format version for forward-compat. Loaders ignore mismatched versions.
    pub version: u32,
    /// Unix timestamp of when this snapshot was generated.
    pub generated_at_unix: u64,
    /// Human-readable timestamp for the freshness banner. Computed at save
    /// time so the frontend doesn't need to do its own date formatting on
    /// the critical path.
    pub generated_at_display: String,
    /// The actual briefing payload — same shape the live morning-briefing
    /// flow produces.
    pub briefing: BriefingNotification,
}

impl BriefingSnapshot {
    /// Compute the snapshot's age in seconds, relative to wall-clock now.
    /// Returns 0 if the system clock has gone backwards (defensive).
    pub fn age_secs(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now.saturating_sub(self.generated_at_unix)
    }

    /// Whether this snapshot is fresh enough to display.
    pub fn is_fresh(&self) -> bool {
        self.version == SNAPSHOT_VERSION && self.age_secs() <= MAX_AGE_SECS
    }
}

/// Resolve the on-disk snapshot path.
///
/// Lives next to the database so cross-platform path resolution is shared.
/// An optional `FOURDA_BRIEFING_SNAPSHOT_PATH` override exists for tests that
/// need to write to a temp dir without poisoning the shared `FOURDA_DB_PATH`
/// env var (which other tests rely on for their own DB resolution).
fn snapshot_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("FOURDA_BRIEFING_SNAPSHOT_PATH") {
        return PathBuf::from(override_path);
    }
    let mut path = get_db_path();
    path.set_file_name("briefing_snapshot.json");
    path
}

/// Defense-in-depth: an abstention synthesis ("low signal" / "no noteworthy
/// intelligence") asserts *all clear*. If the briefing nonetheless carries real
/// content — items, preemption alerts, or escalating chains — that pairing is
/// self-contradictory and would surface a false all-clear as the cold-boot
/// headline (the failure observed 2026-06-19: a briefing baked before the
/// un-abstain fix stored a "Low signal" synthesis on top of 6 items + 5 high
/// CVEs). In that case we clear the synthesis to `None` so the next cold boot
/// renders the real items instead of hiding them behind a "nothing to report"
/// verdict. A stand-alone abstention (no items/alerts/chains — e.g. a
/// knowledge-gap-only brief) is legitimate and passes through untouched. This
/// is a safety net beneath the synthesizer's own grounding gate, not a
/// replacement for it.
fn drop_contradictory_abstention(mut briefing: BriefingNotification) -> BriefingNotification {
    let has_real_content = !briefing.items.is_empty()
        || !briefing.preemption_alerts.is_empty()
        || !briefing.escalating_chains.is_empty();
    let is_abstention = briefing
        .synthesis
        .as_deref()
        .is_some_and(crate::monitoring_briefing::is_abstention_synthesis);
    if has_real_content && is_abstention {
        warn!(
            target: "4da::briefing_snapshot",
            items = briefing.items.len(),
            alerts = briefing.preemption_alerts.len(),
            chains = briefing.escalating_chains.len(),
            "Dropped contradictory abstention synthesis from snapshot — briefing carries real content"
        );
        briefing.synthesis = None;
    }
    briefing
}

/// Save the latest briefing to disk. Best-effort — failures are logged
/// but never propagate. Atomic via temp file + rename.
///
/// Called from:
/// - Successful morning briefing generation (steady state, every cycle)
/// - The `Stop` event handler in `app_setup::handle_run_event` so we
///   capture the latest brief at shutdown
/// - The `complete_scheduled_check` flow when a new briefing is computed
pub fn save_snapshot(briefing: &BriefingNotification) {
    // Refuse to save snapshots with no meaningful content — they would just
    // produce a blank screen on next boot, defeating the entire point.
    if !briefing.has_meaningful_content() {
        debug!(target: "4da::briefing_snapshot", "Skipping snapshot save — no meaningful content");
        return;
    }

    // Never persist an "all clear" abstention on top of real content.
    let briefing = drop_contradictory_abstention(briefing.clone());
    let item_count = briefing.items.len();
    let had_synthesis = briefing.synthesis.is_some();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let display = chrono::Local::now()
        .format("%a %b %-d, %-I:%M %p")
        .to_string();

    let snapshot = BriefingSnapshot {
        version: SNAPSHOT_VERSION,
        generated_at_unix: now,
        generated_at_display: display,
        briefing,
    };

    let json = match serde_json::to_string(&snapshot) {
        Ok(j) => j,
        Err(e) => {
            warn!(target: "4da::briefing_snapshot", error = %e, "Snapshot serialize failed");
            return;
        }
    };

    let path = snapshot_path();
    let parent = match path.parent() {
        Some(p) => p,
        None => {
            warn!(target: "4da::briefing_snapshot", "Snapshot path has no parent");
            return;
        }
    };

    if let Err(e) = std::fs::create_dir_all(parent) {
        warn!(target: "4da::briefing_snapshot", error = %e, "Could not create snapshot dir");
        return;
    }

    // Atomic write: temp file in the same directory + rename. Same-dir
    // is required for `rename` to be atomic on Windows.
    let temp_path = path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&temp_path, json.as_bytes()) {
        warn!(target: "4da::briefing_snapshot", error = %e, "Snapshot temp write failed");
        return;
    }
    if let Err(e) = std::fs::rename(&temp_path, &path) {
        warn!(target: "4da::briefing_snapshot", error = %e, "Snapshot rename failed");
        // Best-effort cleanup of the temp file
        let _ = std::fs::remove_file(&temp_path);
        return;
    }

    info!(
        target: "4da::briefing_snapshot",
        items = item_count,
        synthesis = had_synthesis,
        path = %path.display(),
        "Briefing snapshot saved"
    );
}

/// Load the on-disk snapshot. Returns `None` if missing, corrupt, expired,
/// or version-mismatched. Never propagates errors — the frontend falls
/// through to its normal empty state if `None` is returned.
pub fn load_snapshot() -> Option<BriefingSnapshot> {
    let path = snapshot_path();

    let bytes = std::fs::read(&path).ok()?;

    let snapshot: BriefingSnapshot = match serde_json::from_slice(&bytes) {
        Ok(s) => s,
        Err(e) => {
            debug!(
                target: "4da::briefing_snapshot",
                error = %e,
                "Snapshot deserialize failed (corrupt or older format) — ignoring"
            );
            return None;
        }
    };

    if !snapshot.is_fresh() {
        debug!(
            target: "4da::briefing_snapshot",
            age_hours = snapshot.age_secs() / 3600,
            version = snapshot.version,
            expected_version = SNAPSHOT_VERSION,
            "Snapshot expired or version mismatch — ignoring"
        );
        return None;
    }

    debug!(
        target: "4da::briefing_snapshot",
        age_min = snapshot.age_secs() / 60,
        items = snapshot.briefing.items.len(),
        "Loaded fresh briefing snapshot"
    );

    Some(snapshot)
}

/// Tauri command: returns the cached briefing snapshot for instant render.
///
/// Called from `main.tsx` BEFORE React mounts. The frontend uses the result
/// to render its first paint, then triggers a background refresh. This is
/// the entry point that turns 4DA from "fast" to "instant" on cold boot.
#[tauri::command]
pub async fn get_briefing_snapshot() -> Result<Option<BriefingSnapshot>, String> {
    Ok(load_snapshot())
}

// Note: an explicit `save_briefing_snapshot_now` Tauri command was considered
// here, but the Stop event handler in `app_setup.rs` already reconstructs a
// briefing from `AnalysisState` and calls `save_snapshot` directly. Adding
// an unused command would create a ghost-command IPC violation.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serialize tests that mutate the global FOURDA_DB_PATH env var.
    /// Without this, parallel test execution races and can leak the
    /// production data path between tests, causing the snapshot file to
    /// land in the real `data/` directory (we hit this in the first
    /// real cold-boot test — leftover "Test Brief" data appeared in
    /// production state and would have shown to the user).
    static ENV_VAR_LOCK: Mutex<()> = Mutex::new(());

    fn fake_briefing(items: usize) -> BriefingNotification {
        use crate::monitoring_briefing::BriefingItem;
        BriefingNotification {
            title: "Test Brief".into(),
            items: (0..items)
                .map(|i| BriefingItem {
                    title: format!("Item {i}"),
                    source_type: "hn".into(),
                    score: 0.5,
                    signal_type: None,
                    url: None,
                    item_id: Some(i as i64),
                    signal_priority: None,
                    description: None,
                    matched_deps: vec![],
                    content_type: None,
                    corroboration_count: 0,
                    alt_sources: vec![],
                    section: None,
                    triage_reason: None,
                })
                .collect(),
            total_relevant: items,
            ongoing_topics: vec![],
            knowledge_gaps: vec![],
            escalating_chains: vec![],
            synthesis: Some("Test synthesis paragraph.".into()),
            preemption_alerts: vec![],
            blind_spot_score: None,
            labels: None,
            personalization_context: None,
            data_freshness: None,
            corroboration_available: false,
            coverage_building: false,
            synthesis_hint: None,
        }
    }

    #[test]
    fn snapshot_with_items_is_fresh() {
        let snapshot = BriefingSnapshot {
            version: SNAPSHOT_VERSION,
            generated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            generated_at_display: "now".into(),
            briefing: fake_briefing(3),
        };
        assert!(snapshot.is_fresh());
    }

    #[test]
    fn old_snapshot_is_not_fresh() {
        let snapshot = BriefingSnapshot {
            version: SNAPSHOT_VERSION,
            generated_at_unix: 0, // epoch — definitely too old
            generated_at_display: "ancient".into(),
            briefing: fake_briefing(1),
        };
        assert!(!snapshot.is_fresh());
        // Age should be enormous
        assert!(snapshot.age_secs() > MAX_AGE_SECS);
    }

    #[test]
    fn version_mismatch_is_not_fresh() {
        let snapshot = BriefingSnapshot {
            version: SNAPSHOT_VERSION + 1,
            generated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            generated_at_display: "now".into(),
            briefing: fake_briefing(1),
        };
        assert!(!snapshot.is_fresh());
    }

    #[test]
    fn empty_briefing_does_not_save() {
        // Verify save_snapshot early-returns on empty (we check by ensuring
        // no snapshot file appears).
        //
        // Uses the dedicated FOURDA_BRIEFING_SNAPSHOT_PATH override (NOT
        // FOURDA_DB_PATH) so this test cannot pollute the production data
        // directory even if other tests are racing on the shared db env var.
        // Serialized via ENV_VAR_LOCK as belt-and-braces.
        let _guard = ENV_VAR_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = std::env::temp_dir().join(format!("4da_test_empty_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let snapshot_file = tmp.join("briefing_snapshot.json");
        std::env::set_var("FOURDA_BRIEFING_SNAPSHOT_PATH", &snapshot_file);

        let _ = std::fs::remove_file(&snapshot_file);

        save_snapshot(&fake_briefing(0));
        assert!(
            !snapshot_file.exists(),
            "save_snapshot should refuse empty briefings"
        );

        std::env::remove_var("FOURDA_BRIEFING_SNAPSHOT_PATH");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_then_load_round_trip() {
        // Uses the dedicated FOURDA_BRIEFING_SNAPSHOT_PATH override so this
        // test cannot accidentally write to the production data directory
        // (the original first-cold-boot test caught a leak from FOURDA_DB_PATH
        // racing with parallel tests — fix is two-layer: dedicated env var +
        // mutex serialization).
        let _guard = ENV_VAR_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = std::env::temp_dir().join(format!("4da_test_rt_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let snapshot_file = tmp.join("briefing_snapshot.json");
        std::env::set_var("FOURDA_BRIEFING_SNAPSHOT_PATH", &snapshot_file);

        save_snapshot(&fake_briefing(2));
        let loaded = load_snapshot();
        assert!(loaded.is_some());
        let snapshot = loaded.unwrap();
        assert_eq!(snapshot.briefing.items.len(), 2);
        assert!(snapshot.is_fresh());
        assert_eq!(
            snapshot.briefing.synthesis.as_deref(),
            Some("Test synthesis paragraph.")
        );

        std::env::remove_var("FOURDA_BRIEFING_SNAPSHOT_PATH");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ------------------------------------------------------------------
    // Contradictory-abstention guard (drop_contradictory_abstention)
    // ------------------------------------------------------------------

    const ABSTENTION: &str = "Low signal -- no noteworthy intelligence overnight.";

    fn briefing_with_synthesis(items: usize, synthesis: Option<&str>) -> BriefingNotification {
        let mut b = fake_briefing(items);
        b.synthesis = synthesis.map(|s| s.to_string());
        b
    }

    #[test]
    fn guard_drops_abstention_when_items_present() {
        // An "all clear" verdict on top of real items is self-contradictory —
        // strip it so the cold boot shows the items, not a false "nothing here".
        let out = drop_contradictory_abstention(briefing_with_synthesis(3, Some(ABSTENTION)));
        assert_eq!(out.synthesis, None);
        assert_eq!(out.items.len(), 3, "items must be retained");
    }

    #[test]
    fn guard_drops_abstention_with_em_dash_variant() {
        // Detector tolerates the em-dash / "no new" shapes the LLM may emit.
        let out = drop_contradictory_abstention(briefing_with_synthesis(
            1,
            Some("Low signal \u{2014} no new intelligence overnight."),
        ));
        assert_eq!(out.synthesis, None);
    }

    #[test]
    fn guard_drops_abstention_when_only_preemption_alerts() {
        // No items, but a high CVE alert is categorically noteworthy — the
        // alert alone is "real content", so the abstention must go.
        let mut b = briefing_with_synthesis(0, Some(ABSTENTION));
        b.preemption_alerts
            .push(crate::monitoring_briefing::BriefingPreemptionAlert::default());
        let out = drop_contradictory_abstention(b);
        assert_eq!(out.synthesis, None);
    }

    #[test]
    fn guard_keeps_real_synthesis_with_items() {
        // A genuine narrative must never be stripped (no false positives).
        let real = "Three things to know: patch axios, watch Tokio, ship the brief.";
        let out = drop_contradictory_abstention(briefing_with_synthesis(2, Some(real)));
        assert_eq!(out.synthesis.as_deref(), Some(real));
    }

    #[test]
    fn guard_keeps_standalone_abstention() {
        // A legitimately quiet brief whose only signal is an aging knowledge
        // gap (no items/alerts/chains) may keep its abstention — there is no
        // contradiction to resolve.
        let mut b = briefing_with_synthesis(0, Some(ABSTENTION));
        b.knowledge_gaps
            .push(crate::monitoring_briefing::KnowledgeGap {
                topic: "rust".into(),
                days_since_last: 30,
            });
        let out = drop_contradictory_abstention(b);
        assert_eq!(out.synthesis.as_deref(), Some(ABSTENTION));
    }

    #[test]
    fn save_snapshot_strips_contradictory_abstention_end_to_end() {
        // Full persistence path: a "Low signal" brief carrying real items must
        // round-trip with synthesis cleared, so the loaded snapshot the
        // frontend reads on cold boot never shows a false all-clear.
        let _guard = ENV_VAR_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let tmp = std::env::temp_dir().join(format!("4da_test_abst_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let snapshot_file = tmp.join("briefing_snapshot.json");
        std::env::set_var("FOURDA_BRIEFING_SNAPSHOT_PATH", &snapshot_file);

        save_snapshot(&briefing_with_synthesis(2, Some(ABSTENTION)));
        let loaded = load_snapshot().expect("snapshot should still be saved (it has items)");
        assert_eq!(loaded.briefing.items.len(), 2);
        assert_eq!(
            loaded.briefing.synthesis, None,
            "contradictory abstention must be stripped before persistence"
        );

        std::env::remove_var("FOURDA_BRIEFING_SNAPSHOT_PATH");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Whether the advisory data behind Preemption's "no known vulnerabilities"
//! is in place.
//!
//! An empty Preemption worklist is a claim: every installed version 4DA read
//! was matched against published advisories and none applied. That claim
//! holds only when (1) there is an inventory to check, (2) every OSV
//! ecosystem in it has a successful advisory sync, (3) that sync is recent
//! enough to trust, and (4) the feed was computed after it. Until now the
//! view inferred all of this from a fetched-item count (>= 100), which a new
//! user with only stack sources (AD-054) and a small project may never reach,
//! and which says nothing about whether the advisory sync ever succeeded.
//!
//! [`AdvisorySync`] states which of those holds. It is stamped on the feed at
//! compute time (what the matching saw) and re-read at serve time (what is
//! true now); [`overlay`] combines the two.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::types::SyncStatus;
use crate::db::Database;

/// A successful sync older than this no longer supports an all-clear. The
/// sync re-runs every 6 h ([`super::sync::DEFAULT_SYNC_MAX_AGE_HOURS`]); three
/// days is the same horizon after which source data counts as stale
/// (`DataFreshness::is_stale`), so a weekend offline still reads as current.
pub(crate) const TRUST_HORIZON_HOURS: i64 = 72;

/// What the advisory data behind an empty worklist supports.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum AdvisorySyncState {
    /// No dependency in an ecosystem OSV covers: nothing could be checked.
    NoInventory,
    /// An ecosystem in the inventory has never been synced.
    Syncing,
    /// Advisories are synced, but this feed was computed before they were;
    /// the recompute that matches against them has not landed yet.
    Matching,
    /// The latest sync of an ecosystem in the inventory failed.
    Failed,
    /// Every ecosystem synced, but the oldest sync is past the trust horizon.
    Stale,
    /// Every ecosystem synced successfully within the trust horizon.
    Ready,
}

/// Readiness of the advisory data an empty Preemption worklist rests on.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export, export_to = "bindings/")]
pub struct AdvisorySync {
    pub state: AdvisorySyncState,
    /// OSV ecosystems the dependency inventory needs advisories for.
    pub ecosystems: u32,
    /// Oldest last-sync time among those ecosystems (UTC,
    /// `YYYY-MM-DD HH:MM:SS`): the freshness floor of the advisory data.
    pub synced_at: Option<String>,
    /// The failing ecosystem's error, when `state` is `failed`.
    pub error: Option<String>,
}

const SYNC_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Classify the sync rows for the ecosystems the inventory needs. Pure: the
/// caller supplies `now`. A failure outranks a missing sync, which outranks
/// staleness — the most actionable problem is the one reported.
pub(crate) fn classify(
    required: &[String],
    statuses: &[SyncStatus],
    now: NaiveDateTime,
) -> AdvisorySync {
    let ecosystems = u32::try_from(required.len()).unwrap_or(u32::MAX);
    let mut out = AdvisorySync {
        state: AdvisorySyncState::Ready,
        ecosystems,
        synced_at: None,
        error: None,
    };
    if required.is_empty() {
        out.state = AdvisorySyncState::NoInventory;
        return out;
    }
    let mut never_synced = false;
    let mut oldest: Option<NaiveDateTime> = None;
    for ecosystem in required {
        let status = statuses
            .iter()
            .find(|s| s.ecosystem.eq_ignore_ascii_case(ecosystem));
        let Some(status) = status else {
            never_synced = true;
            continue;
        };
        if let Some(err) = &status.error {
            if out.error.is_none() {
                out.error = Some(format!("{}: {err}", status.ecosystem));
            }
            continue;
        }
        match status
            .last_synced_at
            .as_deref()
            .and_then(|t| NaiveDateTime::parse_from_str(t, SYNC_TIME_FORMAT).ok())
        {
            Some(at) => oldest = Some(oldest.map_or(at, |o| o.min(at))),
            None => never_synced = true,
        }
    }
    out.synced_at = oldest.map(|t| t.format(SYNC_TIME_FORMAT).to_string());
    out.state = if out.error.is_some() {
        AdvisorySyncState::Failed
    } else if never_synced {
        AdvisorySyncState::Syncing
    } else if oldest
        .is_some_and(|o| now.signed_duration_since(o).num_hours() >= TRUST_HORIZON_HOURS)
    {
        AdvisorySyncState::Stale
    } else {
        AdvisorySyncState::Ready
    };
    out
}

/// The live readiness: the OSV ecosystems of the auditable inventory (the
/// same set the sync and the matcher read) against the sync status rows.
/// An unreadable inventory or status table reads as `Syncing` — never as
/// `Ready`.
pub(crate) fn advisory_sync(db: &Database) -> AdvisorySync {
    let required: Vec<String> = match super::sync::auditable_ecosystems(db) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(target: "4da::osv", error = %e, "advisory readiness: inventory unreadable");
            return unreadable();
        }
    };
    let statuses = match db.get_osv_sync_statuses() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: "4da::osv", error = %e, "advisory readiness: sync status unreadable");
            return unreadable();
        }
    };
    classify(&required, &statuses, chrono::Utc::now().naive_utc())
}

fn unreadable() -> AdvisorySync {
    AdvisorySync {
        state: AdvisorySyncState::Syncing,
        ecosystems: 0,
        synced_at: None,
        error: None,
    }
}

/// Combine what the feed's matching saw (`stamped`, from compute time) with
/// what is true now (`live`). A problem now always wins. When the data is
/// ready now but was not when the feed was computed, the feed has not been
/// matched against it yet: `Matching`, not `Ready`. A feed with no stamp (a
/// snapshot from an older build) is judged on the live state alone.
pub(crate) fn overlay(stamped: Option<&AdvisorySync>, live: AdvisorySync) -> AdvisorySync {
    match stamped {
        Some(s)
            if live.state == AdvisorySyncState::Ready && s.state != AdvisorySyncState::Ready =>
        {
            AdvisorySync {
                state: AdvisorySyncState::Matching,
                ..live
            }
        }
        _ => live,
    }
}

#[cfg(test)]
#[path = "readiness_tests.rs"]
mod tests;

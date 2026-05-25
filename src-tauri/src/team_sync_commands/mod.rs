// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Tauri commands for team sync UI integration.
//!
//! Phase 4: Status, members, sharing commands.
//! Phase 4b: Decision voting & retrieval.
//! Phase 5: Team creation, invite flow, key exchange.
//! Phase 6: Shared source management.

mod decisions;
mod lifecycle;
mod sharing;
mod sources;

// Re-export all public Tauri commands so callers can use
// `team_sync_commands::command_name` unchanged.
pub use decisions::{get_decision_detail, get_team_decisions, resolve_decision, vote_on_decision};
pub use lifecycle::{create_team, create_team_invite, join_team_via_invite};
pub use sharing::{
    get_team_members, get_team_sync_status, propose_team_decision, share_dna_with_team,
    share_signal_with_team,
};
pub use sources::{
    get_team_sources, remove_team_source, share_source_with_team, upvote_team_source,
};

// ============================================================================
// Helpers (shared across submodules)
// ============================================================================

use crate::error::Result;
use crate::team_sync;
use crate::team_sync_types::*;

/// Extract team_id and client_id from settings, validating that team sync is
/// enabled and configured. Returns an error suitable for Tauri command results.
pub(crate) fn get_team_config() -> Result<(String, String)> {
    let settings = crate::state::get_settings_manager().lock();
    let config = settings
        .get()
        .team_relay
        .as_ref()
        .ok_or("Team sync not configured")?;

    if !config.enabled {
        return Err("Team sync is not enabled".into());
    }

    let team_id = config
        .team_id
        .as_ref()
        .ok_or("No team ID configured")?
        .clone();
    let client_id = config
        .client_id
        .as_ref()
        .ok_or("No client ID configured")?
        .clone();

    Ok((team_id, client_id))
}

/// Queue a TeamOp for outbound sync with the current HLC timestamp.
pub(crate) fn queue_team_op(team_id: &str, client_id: &str, op: &TeamOp) -> Result<String> {
    let hlc_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let conn = crate::state::open_db_connection()?;
    let entry_id =
        team_sync::queue_entry(&conn, team_id, client_id, hlc_ts, op).map_err(|e| e.to_string())?;

    Ok(entry_id)
}

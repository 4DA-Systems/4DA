//! Team sync scheduler -- background task that syncs metadata with the relay.
//!
//! Integrates with the existing monitoring scheduler pattern (see `monitoring.rs`).
//! Push pending -> Pull new -> Apply inbound, every 30 seconds by default.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Runtime};
use tracing::{debug, info, warn};

use crate::team_sync;
use crate::team_sync_crypto;
use crate::team_sync_types::TeamRelayConfig;

// ============================================================================
// Shared State
// ============================================================================

/// Shared state for the team sync scheduler.
///
/// All fields use atomic types or `parking_lot::Mutex` for lock-free reads
/// from the scheduler loop and safe writes from Tauri command handlers.
pub struct TeamSyncState {
    pub enabled: AtomicBool,
    pub connected: AtomicBool,
    pub last_sync: AtomicU64,
    pub team_id: parking_lot::Mutex<Option<String>>,
    pub client_id: parking_lot::Mutex<Option<String>>,
    pub relay_url: parking_lot::Mutex<Option<String>>,
    pub auth_token: parking_lot::Mutex<Option<String>>,
    pub team_key: parking_lot::Mutex<Option<[u8; 32]>>,
    pub sync_interval_secs: AtomicU64,
}

impl Default for TeamSyncState {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            last_sync: AtomicU64::new(0),
            team_id: parking_lot::Mutex::new(None),
            client_id: parking_lot::Mutex::new(None),
            relay_url: parking_lot::Mutex::new(None),
            auth_token: parking_lot::Mutex::new(None),
            team_key: parking_lot::Mutex::new(None),
            sync_interval_secs: AtomicU64::new(30),
        }
    }
}

impl TeamSyncState {
    /// Configure from a `TeamRelayConfig` (loaded from settings).
    pub fn configure(&self, config: &TeamRelayConfig) {
        self.enabled.store(config.enabled, Ordering::Relaxed);
        *self.team_id.lock() = config.team_id.clone();
        *self.client_id.lock() = config.client_id.clone();
        *self.relay_url.lock() = config.relay_url.clone();
        *self.auth_token.lock() = config.auth_token.clone();
        self.sync_interval_secs
            .store(config.sync_interval_secs.unwrap_or(30), Ordering::Relaxed);
    }

    /// Check if sync is properly configured (has all required fields).
    pub fn is_configured(&self) -> bool {
        self.team_id.lock().is_some()
            && self.client_id.lock().is_some()
            && self.relay_url.lock().is_some()
            && self.auth_token.lock().is_some()
            && self.team_key.lock().is_some()
    }
}

// ============================================================================
// Relay API Response Types
// ============================================================================

/// Response from relay `POST /teams/{team_id}/entries`.
#[derive(Deserialize)]
struct PushResponse {
    #[allow(dead_code)]
    relay_seq: i64,
}

/// Response from relay `GET /teams/{team_id}/entries?since=N`.
#[derive(Deserialize)]
struct PullResponse {
    entries: Vec<RelayEntry>,
    #[allow(dead_code)]
    has_more: bool,
}

/// A single entry returned from the relay.
#[derive(Deserialize)]
struct RelayEntry {
    relay_seq: i64,
    client_id: String,
    payload: Vec<u8>,
}

// ============================================================================
// Sync Stats
// ============================================================================

struct SyncStats {
    pushed: usize,
    pulled: usize,
    applied: usize,
}

// ============================================================================
// Background Scheduler
// ============================================================================

/// Start the background team sync scheduler.
///
/// Follows the same pattern as `monitoring::start_scheduler`:
/// - Spawns a tokio task via `tauri::async_runtime::spawn`
/// - Sleeps on an interval between cycles
/// - Never panics -- all errors are logged and retried next cycle
pub fn start_sync_scheduler<R: Runtime>(app: AppHandle<R>, state: Arc<TeamSyncState>) {
    info!(target: "4da::team_sync", "Starting team sync scheduler");

    tauri::async_runtime::spawn(async move {
        let http = reqwest::Client::builder()
            .user_agent("4DA-TeamSync/1.0")
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // Wait 10 seconds on startup before first sync attempt,
        // giving the app time to initialize settings and database.
        tokio::time::sleep(Duration::from_secs(10)).await;

        loop {
            let interval = state.sync_interval_secs.load(Ordering::Relaxed);
            // Enforce minimum 5-second interval to prevent tight loops
            tokio::time::sleep(Duration::from_secs(interval.max(5))).await;

            if !state.enabled.load(Ordering::Relaxed) || !state.is_configured() {
                continue;
            }

            match run_sync_cycle(&http, &state).await {
                Ok(stats) => {
                    state.connected.store(true, Ordering::Relaxed);
                    state.last_sync.store(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        Ordering::Relaxed,
                    );

                    if stats.pushed > 0 || stats.pulled > 0 {
                        debug!(
                            target: "4da::team_sync",
                            pushed = stats.pushed,
                            pulled = stats.pulled,
                            applied = stats.applied,
                            "Sync cycle complete"
                        );

                        // Notify frontend of sync activity
                        let _ = app.emit(
                            "team-sync-update",
                            serde_json::json!({
                                "pushed": stats.pushed,
                                "pulled": stats.pulled,
                                "applied": stats.applied,
                            }),
                        );
                    }
                }
                Err(e) => {
                    state.connected.store(false, Ordering::Relaxed);
                    warn!(
                        target: "4da::team_sync",
                        error = %e,
                        "Sync cycle failed (will retry next interval)"
                    );
                }
            }
        }
    });
}

// ============================================================================
// Sync Cycle
// ============================================================================

/// Run one complete sync cycle: push -> pull -> apply.
///
/// Snapshot all config values at the top to minimize lock hold times.
/// Each sub-phase opens its own short-lived DB connection.
async fn run_sync_cycle(http: &reqwest::Client, state: &TeamSyncState) -> Result<SyncStats> {
    // Snapshot config from state (locks released immediately)
    let team_id = state.team_id.lock().clone().unwrap_or_default();
    let client_id = state.client_id.lock().clone().unwrap_or_default();
    let relay_url = state.relay_url.lock().clone().unwrap_or_default();
    let auth_token = state.auth_token.lock().clone().unwrap_or_default();
    let team_key = state.team_key.lock().unwrap_or([0u8; 32]);

    if team_id.is_empty() || relay_url.is_empty() {
        return Ok(SyncStats {
            pushed: 0,
            pulled: 0,
            applied: 0,
        });
    }

    // Phase 1: Push pending local entries to the relay
    let pushed = push_pending(
        http,
        &relay_url,
        &team_id,
        &client_id,
        &auth_token,
        &team_key,
    )
    .await?;

    // Phase 2: Pull new entries from the relay since last known sequence
    let (pulled, new_entries) = pull_new(http, &relay_url, &team_id, &auth_token).await?;

    // Phase 3: Apply inbound entries to local database
    let applied = apply_inbound(&team_id, &team_key, &new_entries)?;

    Ok(SyncStats {
        pushed,
        pulled,
        applied,
    })
}

// ============================================================================
// Push Phase
// ============================================================================

/// Push all pending local entries to the relay, encrypted.
///
/// Entries are read from `team_sync_queue` where `acked_at IS NULL`,
/// encrypted with the team key, then POSTed to the relay.
/// Successfully pushed entries are marked as acknowledged.
async fn push_pending(
    http: &reqwest::Client,
    relay_url: &str,
    team_id: &str,
    client_id: &str,
    auth_token: &str,
    team_key: &[u8; 32],
) -> Result<usize> {
    let conn = crate::state::open_db_connection()
        .map_err(|e| anyhow::anyhow!("Failed to open DB for push: {e}"))?;
    let pending = team_sync::get_pending_entries(&conn, team_id)?;

    if pending.is_empty() {
        return Ok(0);
    }

    let mut pushed = 0;
    let mut acked_ids = Vec::new();

    for (entry_id, raw_data) in &pending {
        // Encrypt the payload if it looks like raw JSON (starts with `{`).
        // Already-encrypted blobs pass through unchanged.
        let payload = if raw_data.first() == Some(&b'{') {
            team_sync_crypto::encrypt_metadata(team_key, raw_data)?
        } else {
            raw_data.clone()
        };

        let url = format!("{}/teams/{}/entries", relay_url, team_id);
        let body = serde_json::json!({
            "client_id": client_id,
            "payload": payload,
        });

        match http
            .post(&url)
            .header("Authorization", format!("Bearer {}", auth_token))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                acked_ids.push(entry_id.clone());
                pushed += 1;
            }
            Ok(resp) => {
                warn!(
                    target: "4da::team_sync",
                    status = %resp.status(),
                    entry_id = %entry_id,
                    "Relay rejected entry"
                );
            }
            Err(e) => {
                warn!(target: "4da::team_sync", error = %e, "Push failed (network)");
                // Stop pushing on network error -- retry next cycle
                break;
            }
        }
    }

    // Mark successfully pushed entries as acknowledged
    if !acked_ids.is_empty() {
        let ack_conn = crate::state::open_db_connection()
            .map_err(|e| anyhow::anyhow!("Failed to open DB for ack: {e}"))?;
        team_sync::mark_entries_acked(&ack_conn, &acked_ids)?;
    }

    Ok(pushed)
}

// ============================================================================
// Pull Phase
// ============================================================================

/// Pull new entries from the relay since our last known sequence.
///
/// Returns `(total_received, stored_entries)` where `stored_entries` only
/// includes entries that were newly inserted (not duplicates).
async fn pull_new(
    http: &reqwest::Client,
    relay_url: &str,
    team_id: &str,
    auth_token: &str,
) -> Result<(usize, Vec<(i64, String, Vec<u8>)>)> {
    let conn = crate::state::open_db_connection()
        .map_err(|e| anyhow::anyhow!("Failed to open DB for pull: {e}"))?;
    let last_seq = team_sync::get_last_relay_seq(&conn, team_id)?;

    let url = format!(
        "{}/teams/{}/entries?since={}&limit=200",
        relay_url, team_id, last_seq
    );

    let resp = http
        .get(&url)
        .header("Authorization", format!("Bearer {}", auth_token))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Pull failed with status {}", resp.status());
    }

    let pull: PullResponse = resp.json().await?;
    let count = pull.entries.len();

    let mut stored = Vec::new();
    for entry in pull.entries {
        // Open a fresh connection per entry to avoid holding it across iterations.
        // SQLite connections are cheap with WAL mode and busy_timeout.
        let store_conn = crate::state::open_db_connection()
            .map_err(|e| anyhow::anyhow!("Failed to open DB for store: {e}"))?;

        if team_sync::store_inbound_entry(
            &store_conn,
            entry.relay_seq,
            team_id,
            &entry.client_id,
            &entry.payload,
        )? {
            stored.push((entry.relay_seq, entry.client_id, entry.payload));
        }
    }

    Ok((count, stored))
}

// ============================================================================
// Apply Phase
// ============================================================================

/// Apply inbound entries to local team tables (decrypt + apply).
///
/// Uses `team_sync::apply_pending_inbound` which processes all unapplied
/// entries from `team_sync_log` and updates `team_sync_state`.
fn apply_inbound(
    team_id: &str,
    team_key: &[u8; 32],
    entries: &[(i64, String, Vec<u8>)],
) -> Result<usize> {
    if entries.is_empty() {
        return Ok(0);
    }

    let conn = crate::state::open_db_connection()
        .map_err(|e| anyhow::anyhow!("Failed to open DB for apply: {e}"))?;

    let key = *team_key;
    let applied = team_sync::apply_pending_inbound(&conn, team_id, &|encrypted| {
        team_sync_crypto::decrypt_entry(&key, encrypted)
    })?;

    Ok(applied)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_disabled_and_unconfigured() {
        let state = TeamSyncState::default();
        assert!(!state.enabled.load(Ordering::Relaxed));
        assert!(!state.connected.load(Ordering::Relaxed));
        assert_eq!(state.last_sync.load(Ordering::Relaxed), 0);
        assert_eq!(state.sync_interval_secs.load(Ordering::Relaxed), 30);
        assert!(!state.is_configured());
    }

    #[test]
    fn configure_from_relay_config() {
        let state = TeamSyncState::default();
        let config = TeamRelayConfig {
            enabled: true,
            relay_url: Some("https://relay.4da.ai".to_string()),
            auth_token: Some("tok_test".to_string()),
            team_id: Some("team-123".to_string()),
            client_id: Some("client-456".to_string()),
            display_name: Some("Alice".to_string()),
            role: Some("admin".to_string()),
            sync_interval_secs: Some(60),
        };
        state.configure(&config);

        assert!(state.enabled.load(Ordering::Relaxed));
        assert_eq!(state.sync_interval_secs.load(Ordering::Relaxed), 60);
        assert_eq!(state.team_id.lock().as_deref(), Some("team-123"));
        assert_eq!(
            state.relay_url.lock().as_deref(),
            Some("https://relay.4da.ai")
        );

        // Still not configured because team_key is not set
        assert!(!state.is_configured());

        // Set team key
        *state.team_key.lock() = Some([42u8; 32]);
        assert!(state.is_configured());
    }

    #[test]
    fn is_configured_requires_all_fields() {
        let state = TeamSyncState::default();

        // Set all except team_key -- should be false
        *state.team_id.lock() = Some("t".to_string());
        *state.client_id.lock() = Some("c".to_string());
        *state.relay_url.lock() = Some("u".to_string());
        *state.auth_token.lock() = Some("a".to_string());
        assert!(!state.is_configured());

        // Set team_key -- should be true
        *state.team_key.lock() = Some([0u8; 32]);
        assert!(state.is_configured());
    }

    #[test]
    fn configure_uses_default_interval_when_none() {
        let state = TeamSyncState::default();
        let config = TeamRelayConfig {
            enabled: true,
            sync_interval_secs: None,
            ..Default::default()
        };
        state.configure(&config);
        assert_eq!(state.sync_interval_secs.load(Ordering::Relaxed), 30);
    }

    #[test]
    fn configure_can_disable() {
        let state = TeamSyncState::default();
        state.enabled.store(true, Ordering::Relaxed);

        let config = TeamRelayConfig {
            enabled: false,
            ..Default::default()
        };
        state.configure(&config);
        assert!(!state.enabled.load(Ordering::Relaxed));
    }
}

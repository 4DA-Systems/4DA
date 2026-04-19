// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Team sync types — metadata entries that transit the encrypted relay.
//!
//! These types define WHAT gets synced between team members.
//! Raw content never leaves the machine — only lightweight metadata.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A single metadata entry queued for sync with the team relay.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamMetadataEntry {
    /// UUID — idempotency key for deduplication
    pub entry_id: String,
    /// UUID of the team member who created this entry
    pub client_id: String,
    /// Hybrid Logical Clock timestamp for Last-Write-Wins ordering
    pub hlc_timestamp: u64,
    /// The operation to apply
    pub operation: TeamOp,
}

/// Operations that can be synced between team members.
/// Each variant carries only metadata — never raw content or file paths.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum TeamOp {
    // --- DNA sharing ---
    ShareDnaSummary {
        primary_stack: Vec<String>,
        interests: Vec<String>,
        blind_spots: Vec<String>,
        identity_summary: String,
    },

    // --- Signal chains ---
    ShareSignal {
        signal_id: String,
        chain_name: String,
        priority: String,
        tech_topics: Vec<String>,
        suggested_action: String,
    },
    ResolveSignal {
        signal_id: String,
        resolution_notes: String,
    },

    // --- Decisions ---
    ProposeDecision {
        decision_id: String,
        title: String,
        decision_type: String,
        rationale: String,
    },
    VoteOnDecision {
        decision_id: String,
        stance: String,
        rationale: String,
    },

    // --- Context ---
    ShareContextSummary {
        active_topics: Vec<String>,
        tech_scores: Vec<(String, f32)>,
    },

    // --- Team key distribution ---
    /// Admin encrypts team key for a specific member using X25519 DH.
    /// Only the target member can decrypt this.
    DeliverTeamKey {
        target_client_id: String,
        /// XChaCha20Poly1305 encrypted team key (encrypted with DH shared secret)
        encrypted_team_key: Vec<u8>,
    },

    // --- Source sharing ---
    ShareSource {
        source_type: String,
        config_summary: String,
        recommendation: String,
    },

    // --- Team membership ---
    MemberJoined {
        display_name: String,
        role: String,
    },
    MemberLeft {
        reason: String,
    },
    RoleChanged {
        target_member_id: String,
        new_role: String,
    },
}

/// Team relay connection configuration stored in settings.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamRelayConfig {
    /// Whether team sync is enabled
    pub enabled: bool,
    /// Relay server endpoint URL (e.g., "https://relay.4da.ai" or self-hosted)
    pub relay_url: Option<String>,
    /// JWT auth token for relay API (issued after license validation)
    pub auth_token: Option<String>,
    /// Team ID this seat belongs to
    pub team_id: Option<String>,
    /// This seat's client UUID
    pub client_id: Option<String>,
    /// Display name for this team member
    pub display_name: Option<String>,
    /// Role: "admin" or "member"
    pub role: Option<String>,
    /// Sync interval in seconds (default: 30)
    pub sync_interval_secs: Option<u64>,
}

/// Status of the team sync system
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamSyncStatus {
    pub enabled: bool,
    pub connected: bool,
    pub team_id: Option<String>,
    pub client_id: Option<String>,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub member_count: usize,
    pub pending_outbound: usize,
    pub last_sync_at: Option<String>,
    pub last_relay_seq: i64,
}

/// A registered team member visible to all seats
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamMember {
    pub client_id: String,
    pub display_name: String,
    pub role: String,
    pub last_seen: Option<String>,
}

/// A source shared by a team member for others to discover and adopt.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SharedSource {
    pub id: String,
    pub team_id: String,
    pub source_type: String,
    /// JSON string of non-sensitive source configuration
    pub config_summary: String,
    pub recommendation: String,
    pub shared_by: String,
    pub upvotes: u32,
    pub created_at: String,
}

/// A team decision with vote count for list views.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamDecision {
    pub id: String,
    pub team_id: String,
    pub title: String,
    pub decision_type: String,
    pub rationale: String,
    pub proposed_by: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub vote_count: i64,
}

/// Detailed view of a team decision including all votes.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DecisionDetail {
    pub id: String,
    pub team_id: String,
    pub title: String,
    pub decision_type: String,
    pub rationale: String,
    pub proposed_by: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub votes: Vec<DecisionVote>,
}

/// A single vote on a team decision.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DecisionVote {
    pub voter_id: String,
    pub stance: String,
    pub rationale: String,
    pub voted_at: String,
}

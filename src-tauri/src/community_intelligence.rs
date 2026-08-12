// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Community Intelligence — privacy-preserving anonymous pattern sharing.
//!
//! Shares PATTERNS (scoring weights, accuracy metrics), never DATA
//! (content, URLs, identity, preferences, tech stack).
//!
//! The three IPC commands (`get_community_status`,
//! `set_community_intelligence_enabled`, `set_community_frequency`), the
//! `CommunityStatus` response type, and the SHA-256 anonymous-id generator were
//! deleted 2026-08-12: the commands were removed from the Tauri handler for
//! having zero frontend callers, which left every one of them unreachable. Only
//! the persisted config type below is live — `settings::types::Settings` carries
//! it. Git history preserves the removed implementation.

use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommunityIntelligenceConfig {
    pub enabled: bool,
    pub frequency: String, // "weekly" | "monthly"
    pub last_contributed: Option<String>,
    pub anonymous_id: Option<String>,
}

impl Default for CommunityIntelligenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: "weekly".to_string(),
            last_contributed: None,
            anonymous_id: None,
        }
    }
}

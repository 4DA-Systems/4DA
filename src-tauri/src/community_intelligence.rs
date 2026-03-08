//! Community Intelligence — privacy-preserving anonymous pattern sharing.
//!
//! Shares PATTERNS (scoring weights, accuracy metrics), never DATA
//! (content, URLs, identity, preferences, tech stack).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::get_settings_manager;

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

#[derive(Serialize, Clone, Debug)]
pub struct CommunityContribution {
    pub anonymous_id: String,
    pub app_version: String,
    pub contribution_type: String,
    pub payload: ScoringWeightContribution,
}

#[derive(Serialize, Clone, Debug)]
pub struct ScoringWeightContribution {
    pub accuracy_aggregate: f64,
    pub signal_effectiveness: std::collections::HashMap<String, f64>,
    pub total_items_scored: u64,
    pub avg_relevant_ratio: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommunityWeights {
    pub version: u32,
    pub generated_at: String,
    pub contributors: u64,
    pub profile_adjustments: std::collections::HashMap<String, ProfileAdjustment>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileAdjustment {
    pub keyword_boost_multiplier: f64,
    pub signal_type_weights: std::collections::HashMap<String, f64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct CommunityStatus {
    pub enabled: bool,
    pub frequency: String,
    pub last_contributed: Option<String>,
    pub anonymous_id_preview: Option<String>, // first 8 chars only
}

// ============================================================================
// Helpers
// ============================================================================

/// Generate a random anonymous ID using SHA-256 hash of timestamp + process info.
/// No external `uuid` crate needed — produces a 64-char hex string.
fn generate_anonymous_id() -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .to_le_bytes(),
    );
    hasher.update(std::process::id().to_le_bytes());
    // Mix in a counter to avoid collisions if called twice in the same nanosecond
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    hasher.update(
        COUNTER
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_le_bytes(),
    );
    hex::encode(hasher.finalize())
}

// ============================================================================
// Commands
// ============================================================================

/// Get the current community intelligence status
#[tauri::command]
pub async fn get_community_status() -> Result<CommunityStatus> {
    let manager = get_settings_manager();
    let guard = manager.lock();
    let settings = guard.get();

    let config = settings.community_intelligence.clone().unwrap_or_default();

    Ok(CommunityStatus {
        enabled: config.enabled,
        frequency: config.frequency,
        last_contributed: config.last_contributed,
        anonymous_id_preview: config
            .anonymous_id
            .as_ref()
            .map(|id| id[..8.min(id.len())].to_string()),
    })
}

/// Toggle community intelligence on/off
#[tauri::command]
pub async fn set_community_intelligence_enabled(enabled: bool) -> Result<()> {
    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let settings = guard.get_mut();

    let config = settings
        .community_intelligence
        .get_or_insert_with(CommunityIntelligenceConfig::default);
    config.enabled = enabled;

    // Generate anonymous ID on first enable
    if enabled && config.anonymous_id.is_none() {
        config.anonymous_id = Some(generate_anonymous_id());
    }

    guard
        .save()
        .map_err(|e| crate::error::FourDaError::Config(e))?;
    Ok(())
}

/// Set contribution frequency
#[tauri::command]
pub async fn set_community_frequency(frequency: String) -> Result<()> {
    if frequency != "weekly" && frequency != "monthly" {
        return Err(crate::error::FourDaError::Config(
            "Frequency must be 'weekly' or 'monthly'".into(),
        ));
    }

    let manager = get_settings_manager();
    let mut guard = manager.lock();
    let settings = guard.get_mut();

    let config = settings
        .community_intelligence
        .get_or_insert_with(CommunityIntelligenceConfig::default);
    config.frequency = frequency;

    guard
        .save()
        .map_err(|e| crate::error::FourDaError::Config(e))?;
    Ok(())
}

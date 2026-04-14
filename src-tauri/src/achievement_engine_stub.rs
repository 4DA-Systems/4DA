// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `achievement_engine` module when "experimental" feature is disabled.

use crate::db::Database;
use crate::achievement_definitions::AchievementTier;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementUnlocked {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub tier: AchievementTier,
    pub celebration_intensity: f64,
    pub unlocked_at: String,
}

pub fn create_tables(_conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    Ok(())
}

pub fn increment_counter(
    _db: &Database,
    _counter_type: &str,
    _amount: u64,
) -> Vec<AchievementUnlocked> {
    Vec::new()
}

#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
pub fn check_daily_streak(_db: &Database) -> Vec<AchievementUnlocked> {
    Vec::new()
}

#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
pub fn get_achievement_state(_db: &Database) -> serde_json::Value {
    serde_json::json!({"counters": [], "achievements": [], "streak": 0, "last_active": null})
}

#[allow(dead_code)] // Feature-gated: stub active only when "experimental" is disabled
pub fn get_achievements(_db: &Database) -> Vec<AchievementUnlocked> {
    Vec::new()
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Waitlist signup storage — captures Team/Enterprise interest locally.
//!
//! Privacy-first: signups stored in local SQLite, never sent externally.
//! When tiers activate, these contacts are the first to be notified.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitlistEntry {
    pub id: i64,
    pub tier: String,
    pub email: String,
    pub name: Option<String>,
    pub team_size: Option<String>,
    pub company: Option<String>,
    pub role: Option<String>,
    pub source: String,
    pub signed_up_at: String,
}

#[tauri::command]
pub fn save_waitlist_signup(
    tier: String,
    email: String,
    name: Option<String>,
    team_size: Option<String>,
    company: Option<String>,
    role: Option<String>,
) -> Result<serde_json::Value> {
    let db = crate::get_database()?;
    let conn = db.conn.lock();

    conn.execute(
        "INSERT OR IGNORE INTO waitlist_signups (tier, email, name, team_size, company, role, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in-app')",
        rusqlite::params![tier, email, name, team_size, company, role],
    )?;

    info!(target: "4da::waitlist", tier = %tier, email = %email, "Waitlist signup recorded");

    Ok(serde_json::json!({
        "success": true,
        "tier": tier,
        "email": email,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waitlist_entry_serialization() {
        let entry = WaitlistEntry {
            id: 1,
            tier: "team".to_string(),
            email: "dev@company.com".to_string(),
            name: Some("Jane".to_string()),
            team_size: Some("10".to_string()),
            company: Some("Acme".to_string()),
            role: Some("Eng Manager".to_string()),
            source: "in-app".to_string(),
            signed_up_at: "2026-03-19T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["tier"], "team");
        assert_eq!(json["email"], "dev@company.com");
    }
}

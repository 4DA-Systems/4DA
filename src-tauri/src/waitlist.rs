// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Waitlist signup storage — captures Team/Enterprise interest locally.
//!
//! Privacy-first: signups stored in local SQLite, never sent externally.
//! When tiers activate, these contacts are the first to be notified.

use tracing::info;

use crate::error::Result;

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

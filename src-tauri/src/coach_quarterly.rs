//! Quarterly review generation for the STREETS Coach.
//!
//! Extracts the 90-day progress review logic from `coach_nudges` so that
//! module stays under the 600-line limit.

use tracing::info;

use crate::llm::{LLMClient, Message};

// ============================================================================
// Private helpers (local copies — originals live in coach_nudges)
// ============================================================================

/// Ensure coach tables exist (called lazily on first access).
fn ensure_tables(conn: &rusqlite::Connection) -> std::result::Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS coach_nudges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nudge_type TEXT NOT NULL,
            content TEXT NOT NULL,
            dismissed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_coach_nudges_dismissed
            ON coach_nudges(dismissed);

        CREATE TABLE IF NOT EXISTS coach_documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            doc_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );",
    )
}

fn get_llm_client_optional() -> Option<LLMClient> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let provider = guard.get().llm.clone();
    if provider.api_key.is_empty() && provider.provider != "ollama" {
        return None;
    }
    Some(LLMClient::new(provider))
}

// ============================================================================
// Quarterly Review (Background Job)
// ============================================================================

/// Generate a quarterly review document.
/// Assembles playbook progress delta, profile changes, and decisions
/// from the last 90 days into a narrative review.
pub async fn generate_quarterly_review() -> std::result::Result<(), String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    ensure_tables(&conn).map_err(|e| e.to_string())?;

    // Lessons completed in last 90 days
    let lessons_completed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM command_execution_log
             WHERE success = 1
               AND executed_at >= datetime('now', '-90 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Profile facts added in last 90 days
    let profile_changes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sovereign_profile
             WHERE updated_at >= datetime('now', '-90 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Build review content
    let review = match get_llm_client_optional() {
        Some(client) => {
            let prompt = format!(
                "Write a brief quarterly progress review for a developer using STREETS. \
                 Stats: {} lessons completed, {} profile updates in the last 90 days. \
                 2-3 paragraphs. Celebrate wins, suggest focus areas.",
                lessons_completed, profile_changes
            );
            match client
                .complete(
                    "You are a concise developer coach writing a quarterly review.",
                    vec![Message {
                        role: "user".to_string(),
                        content: prompt,
                    }],
                )
                .await
            {
                Ok(resp) => resp.content,
                Err(_) => format_template_review(lessons_completed, profile_changes),
            }
        }
        None => format_template_review(lessons_completed, profile_changes),
    };

    conn.execute(
        "INSERT INTO coach_documents (doc_type, content) VALUES ('quarterly_review', ?1)",
        rusqlite::params![review],
    )
    .map_err(|e| e.to_string())?;

    info!(target: "4da::coach", "Generated quarterly review");
    Ok(())
}

fn format_template_review(lessons: i64, profile_updates: i64) -> String {
    format!(
        "Quarterly Review\n\n\
         Over the last 90 days you completed {} lesson commands and made {} \
         updates to your Sovereign Profile.\n\n\
         {}",
        lessons,
        profile_updates,
        if lessons == 0 {
            "Consider picking up the STREETS playbook again -- even one lesson per week builds momentum."
        } else {
            "Keep the momentum going. Consistency compounds over time."
        }
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_review_zero_lessons() {
        let review = format_template_review(0, 5);
        assert!(review.contains("0 lesson commands"));
        assert!(review.contains("5 updates"));
        assert!(
            review.contains("picking up the STREETS playbook"),
            "zero lessons should encourage restarting"
        );
    }

    #[test]
    fn template_review_nonzero_lessons() {
        let review = format_template_review(10, 3);
        assert!(review.contains("10 lesson commands"));
        assert!(review.contains("3 updates"));
        assert!(
            review.contains("Keep the momentum"),
            "nonzero lessons should encourage momentum"
        );
    }
}

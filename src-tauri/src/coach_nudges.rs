//! STREETS Coach nudge system.
//!
//! Generates AI coaching nudges that appear in the Coach dashboard.
//! Integrates with the monitoring scheduler to run daily, checking for
//! profile gaps, stale progress, and providing actionable suggestions.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::{FourDaError, Result};
use crate::llm::{LLMClient, Message};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachNudge {
    pub id: i64,
    pub nudge_type: String, // "progress", "profile_gap", "engine_suggestion"
    pub content: String,
    pub dismissed: bool,
    pub created_at: String,
}

// ============================================================================
// Table Initialization
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

// ============================================================================
// LLM Helper
// ============================================================================

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
// Template Nudges
// ============================================================================

fn generate_template_nudge(nudge_type: &str) -> String {
    match nudge_type {
        "profile_gap" => "Your Sovereign Profile is incomplete. Complete the profile audit \
             in Module S to get more accurate coaching recommendations."
            .to_string(),
        "progress" => "It's been a while since you worked on STREETS modules. Pick up \
             where you left off -- consistency beats intensity."
            .to_string(),
        "engine_suggestion" => "Ready to pick your first revenue engine? Use the Engine \
             Recommender for a personalized analysis."
            .to_string(),
        _ => "Keep building. The STREETS playbook is your roadmap.".to_string(),
    }
}

// ============================================================================
// Nudge Generation (Background Job)
// ============================================================================

/// Generate a coaching nudge based on user state.
/// Called by the monitoring scheduler daily.
pub async fn generate_nudge() -> std::result::Result<(), String> {
    let conn = crate::open_db_connection().map_err(|e| e.to_string())?;
    ensure_tables(&conn).map_err(|e| e.to_string())?;

    // Limit: max 3 undismissed nudges at a time
    let undismissed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM coach_nudges WHERE dismissed = 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if undismissed >= 3 {
        info!(target: "4da::coach", "Already at nudge limit ({undismissed}), skipping");
        return Ok(());
    }

    // Determine nudge type based on user state
    let nudge_type = determine_nudge_type(&conn);
    let nudge_type = match nudge_type {
        Some(t) => t,
        None => {
            info!(target: "4da::coach", "No nudge conditions triggered");
            return Ok(());
        }
    };

    // Generate nudge content (LLM or template)
    let nudge_text = match get_llm_client_optional() {
        Some(client) => {
            let prompt = format!(
                "Generate a brief 2-3 sentence coaching nudge for a developer \
                 working through the STREETS playbook. Nudge type: '{}'. \
                 Be encouraging, specific, and actionable. No fluff.",
                nudge_type
            );
            match client
                .complete(
                    "You are a concise developer coach.",
                    vec![Message {
                        role: "user".to_string(),
                        content: prompt,
                    }],
                )
                .await
            {
                Ok(resp) => resp.content,
                Err(e) => {
                    warn!(target: "4da::coach", error = %e, "LLM nudge failed, using template");
                    generate_template_nudge(&nudge_type)
                }
            }
        }
        None => generate_template_nudge(&nudge_type),
    };

    conn.execute(
        "INSERT INTO coach_nudges (nudge_type, content) VALUES (?1, ?2)",
        rusqlite::params![nudge_type, nudge_text],
    )
    .map_err(|e| e.to_string())?;

    info!(target: "4da::coach", nudge_type = %nudge_type, "Generated coaching nudge");
    Ok(())
}

/// Check user state and return the most relevant nudge type, or None.
fn determine_nudge_type(conn: &rusqlite::Connection) -> Option<String> {
    // 1. Check sovereign profile completeness (< 50% = profile_gap)
    let category_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT category) FROM sovereign_profile",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // 6 expected categories for a complete profile
    if category_count < 3 {
        return Some("profile_gap".to_string());
    }

    // 2. Check last lesson completion (14+ days ago = progress nudge)
    let last_lesson: Option<String> = conn
        .query_row(
            "SELECT MAX(executed_at) FROM command_execution_log WHERE success = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(None);

    if let Some(last) = last_lesson {
        if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(&last, "%Y-%m-%d %H:%M:%S") {
            let days_ago = (chrono::Utc::now().naive_utc() - parsed).num_days();
            if days_ago >= 14 {
                return Some("progress".to_string());
            }
        }
    } else {
        // No lessons completed at all -- also a progress nudge
        return Some("progress".to_string());
    }

    // 3. Check if user has no coaching sessions -- suggest engine
    let session_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM coach_sessions", [], |row| row.get(0))
        .unwrap_or(0);

    if session_count == 0 {
        return Some("engine_suggestion".to_string());
    }

    None
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
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_coach_nudges() -> Result<Vec<CoachNudge>> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    ensure_tables(&conn).map_err(FourDaError::Db)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, nudge_type, content, dismissed, created_at
             FROM coach_nudges
             WHERE dismissed = 0
             ORDER BY created_at DESC
             LIMIT 10",
        )
        .map_err(FourDaError::Db)?;

    let nudges = stmt
        .query_map([], |row| {
            Ok(CoachNudge {
                id: row.get(0)?,
                nudge_type: row.get(1)?,
                content: row.get(2)?,
                dismissed: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
            })
        })
        .map_err(FourDaError::Db)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(nudges)
}

#[tauri::command]
pub async fn dismiss_coach_nudge(nudge_id: i64) -> Result<()> {
    let conn = crate::open_db_connection().map_err(FourDaError::Internal)?;
    ensure_tables(&conn).map_err(FourDaError::Db)?;

    conn.execute(
        "UPDATE coach_nudges SET dismissed = 1 WHERE id = ?1",
        rusqlite::params![nudge_id],
    )
    .map_err(FourDaError::Db)?;

    info!(target: "4da::coach", nudge_id = nudge_id, "Dismissed coaching nudge");
    Ok(())
}

// ============================================================================
// Monitoring Integration
// ============================================================================

/// Entry point for the monitoring scheduler's daily nudge check.
pub async fn run_daily_nudge_check() {
    if let Err(e) = generate_nudge().await {
        warn!(target: "4da::coach", error = %e, "Daily nudge generation failed");
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Create an in-memory database with all tables that coach_nudges depends on.
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ensure_tables(&conn).expect("create coach tables");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sovereign_profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                raw_output TEXT,
                source_command TEXT,
                source_lesson TEXT,
                confidence REAL DEFAULT 1.0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(category, key)
            );
            CREATE TABLE IF NOT EXISTS command_execution_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                module_id TEXT NOT NULL,
                lesson_idx INTEGER NOT NULL,
                command_id TEXT NOT NULL,
                command_text TEXT NOT NULL,
                success INTEGER NOT NULL,
                exit_code INTEGER,
                stdout TEXT,
                stderr TEXT,
                duration_ms INTEGER,
                executed_at TEXT DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS coach_sessions (
                id TEXT PRIMARY KEY,
                session_type TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT 'New Session',
                context_snapshot TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create supporting tables");
        conn
    }

    // ---- CoachNudge serialization ----

    #[test]
    fn coach_nudge_serializes_to_json() {
        let nudge = CoachNudge {
            id: 1,
            nudge_type: "progress".to_string(),
            content: "Keep going!".to_string(),
            dismissed: false,
            created_at: "2025-01-01 00:00:00".to_string(),
        };
        let json = serde_json::to_string(&nudge).expect("serialize");
        assert!(json.contains("\"nudge_type\":\"progress\""));
        assert!(json.contains("\"dismissed\":false"));
    }

    #[test]
    fn coach_nudge_roundtrip_json() {
        let nudge = CoachNudge {
            id: 42,
            nudge_type: "profile_gap".to_string(),
            content: "Complete your profile.".to_string(),
            dismissed: true,
            created_at: "2025-06-15 12:30:00".to_string(),
        };
        let json = serde_json::to_string(&nudge).expect("serialize");
        let deserialized: CoachNudge = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, 42);
        assert_eq!(deserialized.nudge_type, "profile_gap");
        assert_eq!(deserialized.content, "Complete your profile.");
        assert!(deserialized.dismissed);
        assert_eq!(deserialized.created_at, "2025-06-15 12:30:00");
    }

    // ---- ensure_tables ----

    #[test]
    fn ensure_tables_creates_coach_nudges_table() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ensure_tables(&conn).expect("ensure_tables");
        conn.execute(
            "INSERT INTO coach_nudges (nudge_type, content) VALUES ('test', 'hello')",
            [],
        )
        .expect("insert into coach_nudges");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM coach_nudges", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn ensure_tables_creates_coach_documents_table() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ensure_tables(&conn).expect("ensure_tables");
        conn.execute(
            "INSERT INTO coach_documents (doc_type, content) VALUES ('review', 'test')",
            [],
        )
        .expect("insert into coach_documents");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM coach_documents", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn ensure_tables_is_idempotent() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ensure_tables(&conn).expect("first call");
        ensure_tables(&conn).expect("second call should succeed");
        conn.execute(
            "INSERT INTO coach_nudges (nudge_type, content) VALUES ('test', 'idem')",
            [],
        )
        .expect("insert after double ensure");
    }

    // ---- generate_template_nudge (pure function, all branches) ----

    #[test]
    fn template_nudge_profile_gap() {
        let text = generate_template_nudge("profile_gap");
        assert!(
            text.contains("Sovereign Profile"),
            "profile_gap nudge should mention Sovereign Profile"
        );
    }

    #[test]
    fn template_nudge_progress() {
        let text = generate_template_nudge("progress");
        assert!(
            text.contains("consistency beats intensity"),
            "progress nudge should mention consistency"
        );
    }

    #[test]
    fn template_nudge_engine_suggestion() {
        let text = generate_template_nudge("engine_suggestion");
        assert!(
            text.contains("Engine Recommender"),
            "engine_suggestion nudge should mention Engine Recommender"
        );
    }

    #[test]
    fn template_nudge_unknown_type_returns_default() {
        let text = generate_template_nudge("nonexistent_type");
        assert!(
            text.contains("Keep building"),
            "unknown type should return default nudge"
        );
    }

    // ---- format_template_review (pure function) ----

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

    // ---- determine_nudge_type (DB-driven logic) ----

    #[test]
    fn nudge_type_profile_gap_when_few_categories() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO sovereign_profile (category, key, value) VALUES ('lang', 'rust', 'yes')",
            [],
        )
        .expect("insert");
        conn.execute(
            "INSERT INTO sovereign_profile (category, key, value) VALUES ('tool', 'vim', 'yes')",
            [],
        )
        .expect("insert");
        assert_eq!(determine_nudge_type(&conn), Some("profile_gap".to_string()));
    }

    #[test]
    fn nudge_type_progress_when_no_lessons() {
        let conn = setup_test_db();
        for cat in &["lang", "tool", "framework"] {
            conn.execute(
                "INSERT INTO sovereign_profile (category, key, value) VALUES (?1, 'k', 'v')",
                rusqlite::params![cat],
            )
            .expect("insert");
        }
        assert_eq!(determine_nudge_type(&conn), Some("progress".to_string()));
    }

    #[test]
    fn nudge_type_engine_suggestion_when_no_coach_sessions() {
        let conn = setup_test_db();
        for cat in &["lang", "tool", "framework"] {
            conn.execute(
                "INSERT INTO sovereign_profile (category, key, value) VALUES (?1, 'k', 'v')",
                rusqlite::params![cat],
            )
            .expect("insert");
        }
        conn.execute(
            "INSERT INTO command_execution_log (module_id, lesson_idx, command_id, command_text, success, executed_at)
             VALUES ('S', 1, 'cmd1', 'echo hi', 1, datetime('now'))",
            [],
        )
        .expect("insert lesson");
        assert_eq!(
            determine_nudge_type(&conn),
            Some("engine_suggestion".to_string())
        );
    }

    #[test]
    fn nudge_type_none_when_all_conditions_met() {
        let conn = setup_test_db();
        for cat in &["lang", "tool", "framework"] {
            conn.execute(
                "INSERT INTO sovereign_profile (category, key, value) VALUES (?1, 'k', 'v')",
                rusqlite::params![cat],
            )
            .expect("insert");
        }
        conn.execute(
            "INSERT INTO command_execution_log (module_id, lesson_idx, command_id, command_text, success, executed_at)
             VALUES ('S', 1, 'cmd1', 'echo hi', 1, datetime('now'))",
            [],
        )
        .expect("insert lesson");
        conn.execute(
            "INSERT INTO coach_sessions (id, session_type, title) VALUES ('s1', 'general', 'Test')",
            [],
        )
        .expect("insert session");
        assert_eq!(determine_nudge_type(&conn), None);
    }

    #[test]
    fn nudge_type_progress_when_lesson_is_stale() {
        let conn = setup_test_db();
        for cat in &["lang", "tool", "framework"] {
            conn.execute(
                "INSERT INTO sovereign_profile (category, key, value) VALUES (?1, 'k', 'v')",
                rusqlite::params![cat],
            )
            .expect("insert");
        }
        conn.execute(
            "INSERT INTO command_execution_log (module_id, lesson_idx, command_id, command_text, success, executed_at)
             VALUES ('S', 1, 'cmd1', 'echo hi', 1, datetime('now', '-30 days'))",
            [],
        )
        .expect("insert stale lesson");
        assert_eq!(determine_nudge_type(&conn), Some("progress".to_string()));
    }
}

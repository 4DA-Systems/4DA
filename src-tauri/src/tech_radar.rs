//! Technology Radar — Computed personal tech radar from existing 4DA data
//!
//! Synthesizes a ThoughtWorks-style Technology Radar from domain profile,
//! developer decisions, topic affinities, and source item mentions.
//! This is a computed view — nothing is stored.
//!
//! Computation engine lives in `tech_radar_compute`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Re-export compute engine for crate-level use
pub(crate) use crate::tech_radar_compute::compute_radar;

// Re-export internals used by tests
#[cfg(test)]
use crate::tech_radar_compute::{classify_quadrant, EntryBuilder};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum RadarRing {
    Adopt,
    Trial,
    Assess,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum RadarMovement {
    Up,
    Down,
    Stable,
    New,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum RadarQuadrant {
    Languages,
    Frameworks,
    Tools,
    Platforms,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct RadarEntry {
    pub name: String,
    pub ring: RadarRing,
    pub quadrant: RadarQuadrant,
    pub movement: RadarMovement,
    pub signals: Vec<String>,
    pub decision_ref: Option<i64>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct TechRadar {
    pub generated_at: String,
    pub entries: Vec<RadarEntry>,
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn get_tech_radar() -> Result<TechRadar, String> {
    let conn = crate::open_db_connection()?;
    compute_radar(&conn)
}

#[tauri::command]
pub async fn get_radar_entry(name: String) -> Result<Option<RadarEntry>, String> {
    let conn = crate::open_db_connection()?;
    let radar = compute_radar(&conn)?;
    Ok(radar
        .entries
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(&name)))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn setup_test_db() -> Connection {
        crate::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tech_stack (id INTEGER PRIMARY KEY, technology TEXT NOT NULL UNIQUE);
             CREATE TABLE detected_tech (id INTEGER PRIMARY KEY, name TEXT NOT NULL, category TEXT, confidence REAL DEFAULT 0.5);
             CREATE TABLE explicit_interests (id INTEGER PRIMARY KEY, topic TEXT NOT NULL);
             CREATE TABLE project_dependencies (id INTEGER PRIMARY KEY, project_path TEXT, manifest_type TEXT, package_name TEXT, version TEXT, is_dev INTEGER DEFAULT 0, language TEXT, last_scanned TEXT DEFAULT (datetime('now')), UNIQUE(project_path, package_name));
             CREATE TABLE developer_decisions (id INTEGER PRIMARY KEY AUTOINCREMENT, decision_type TEXT NOT NULL, subject TEXT NOT NULL, decision TEXT NOT NULL, rationale TEXT, alternatives_rejected TEXT DEFAULT '[]', context_tags TEXT DEFAULT '[]', confidence REAL DEFAULT 0.8, status TEXT DEFAULT 'active', superseded_by INTEGER, created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')));
             CREATE TABLE source_items (id INTEGER PRIMARY KEY AUTOINCREMENT, source_type TEXT NOT NULL, source_id TEXT NOT NULL, url TEXT, title TEXT NOT NULL, content TEXT DEFAULT '', content_hash TEXT DEFAULT '', embedding BLOB DEFAULT x'00', created_at TEXT DEFAULT (datetime('now')), last_seen TEXT DEFAULT (datetime('now')), UNIQUE(source_type, source_id));",
        ).unwrap();
        conn
    }

    #[test]
    fn test_classify_quadrant() {
        assert_eq!(classify_quadrant("rust"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("typescript"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("python"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("react"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("tauri"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("django"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("aws"), RadarQuadrant::Platforms);
        assert_eq!(classify_quadrant("vercel"), RadarQuadrant::Platforms);
        assert_eq!(classify_quadrant("docker"), RadarQuadrant::Tools);
        assert_eq!(classify_quadrant("webpack"), RadarQuadrant::Tools);
        assert_eq!(classify_quadrant("obscure-lib"), RadarQuadrant::Tools);
    }

    #[test]
    fn test_compute_radar_with_profile() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tech_stack (technology) VALUES ('typescript')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_dependencies (project_path, manifest_type, package_name, version, is_dev, language)
             VALUES ('/proj', 'cargo', 'serde', '1.0', 0, 'rust')", [],
        ).unwrap();

        let radar = compute_radar(&conn).unwrap();
        assert!(!radar.entries.is_empty());

        let rust = radar.entries.iter().find(|e| e.name == "rust").unwrap();
        assert_eq!(rust.ring, RadarRing::Adopt);
        assert!(rust.score > 0.3);

        let ts = radar
            .entries
            .iter()
            .find(|e| e.name == "typescript")
            .unwrap();
        assert_eq!(ts.ring, RadarRing::Adopt);

        assert!(radar.entries.iter().any(|e| e.name == "serde"));
    }

    #[test]
    fn test_decision_overlay() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('sqlite')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO developer_decisions (decision_type, subject, decision, alternatives_rejected, status)
             VALUES ('tech_choice', 'sqlite', 'Use SQLite', '[\"postgresql\", \"mysql\"]', 'active')", [],
        ).unwrap();

        let radar = compute_radar(&conn).unwrap();

        let sqlite = radar.entries.iter().find(|e| e.name == "sqlite").unwrap();
        assert_eq!(sqlite.ring, RadarRing::Adopt);
        assert!(sqlite.decision_ref.is_some());

        let pg = radar
            .entries
            .iter()
            .find(|e| e.name == "postgresql")
            .unwrap();
        assert_eq!(pg.ring, RadarRing::Hold);
        assert!(pg.signals.iter().any(|s| s.contains("Rejected")));

        let mysql = radar.entries.iter().find(|e| e.name == "mysql").unwrap();
        assert_eq!(mysql.ring, RadarRing::Hold);
    }

    // -- classify_quadrant exhaustive --

    #[test]
    fn classify_quadrant_case_insensitive() {
        assert_eq!(classify_quadrant("Rust"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("RUST"), RadarQuadrant::Languages);
        assert_eq!(classify_quadrant("React"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("AWS"), RadarQuadrant::Platforms);
    }

    #[test]
    fn classify_quadrant_contains_match() {
        // Frameworks use .contains() so substrings match
        assert_eq!(classify_quadrant("my-react-app"), RadarQuadrant::Frameworks);
        assert_eq!(classify_quadrant("vue-router"), RadarQuadrant::Frameworks);
        // Platforms too
        assert_eq!(classify_quadrant("aws-lambda"), RadarQuadrant::Platforms);
    }

    #[test]
    fn classify_quadrant_empty_and_unknown() {
        assert_eq!(classify_quadrant(""), RadarQuadrant::Tools);
        assert_eq!(classify_quadrant("obscure-lib"), RadarQuadrant::Tools);
        assert_eq!(classify_quadrant("my-custom-tool"), RadarQuadrant::Tools);
    }

    // -- EntryBuilder::score --

    #[test]
    fn entry_builder_score_all_zeros() {
        let eb = EntryBuilder::new(RadarRing::Assess, 0.0);
        assert!((eb.score() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn entry_builder_score_all_ones() {
        let mut eb = EntryBuilder::new(RadarRing::Adopt, 1.0);
        eb.engagement = 1.0;
        eb.trend = 1.0;
        eb.decision_boost = 1.0;
        assert!((eb.score() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn entry_builder_score_weights_sum_to_one() {
        // 0.4 + 0.3 + 0.2 + 0.1 = 1.0
        // If all inputs are 1.0, result should be exactly 1.0
        let mut eb = EntryBuilder::new(RadarRing::Adopt, 1.0);
        eb.engagement = 1.0;
        eb.trend = 1.0;
        eb.decision_boost = 1.0;
        assert!((eb.score() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn entry_builder_score_stack_weight_only() {
        let eb = EntryBuilder::new(RadarRing::Trial, 0.9);
        // 0.9 * 0.4 = 0.36
        assert!((eb.score() - 0.36).abs() < 1e-6);
    }

    #[test]
    fn entry_builder_score_clamps_above_one() {
        let mut eb = EntryBuilder::new(RadarRing::Adopt, 3.0);
        eb.engagement = 3.0;
        eb.trend = 3.0;
        eb.decision_boost = 3.0;
        assert!((eb.score() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn entry_builder_into_entry_carries_fields() {
        let mut eb = EntryBuilder::new(RadarRing::Adopt, 0.9);
        eb.quadrant = RadarQuadrant::Languages;
        eb.movement = RadarMovement::Up;
        eb.signals = vec!["trending".to_string()];
        eb.decision_ref = Some(42);
        eb.engagement = 0.5;

        let entry = eb.into_entry("rust".to_string());
        assert_eq!(entry.name, "rust");
        assert_eq!(entry.ring, RadarRing::Adopt);
        assert_eq!(entry.quadrant, RadarQuadrant::Languages);
        assert_eq!(entry.movement, RadarMovement::Up);
        assert_eq!(entry.signals, vec!["trending"]);
        assert_eq!(entry.decision_ref, Some(42));
        assert!(entry.score > 0.0);
    }

    #[test]
    fn test_signal_trends() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
            .unwrap();
        for i in 0..8 {
            conn.execute(
                "INSERT INTO source_items (source_type, source_id, title, content)
                 VALUES ('hackernews', ?1, ?2, 'Rust programming language news')",
                params![format!("hn-{}", i), format!("Rust {} release notes", i)],
            )
            .unwrap();
        }

        let radar = compute_radar(&conn).unwrap();
        let rust = radar.entries.iter().find(|e| e.name == "rust").unwrap();
        assert_eq!(rust.movement, RadarMovement::Up);
        assert!(rust.signals.iter().any(|s| s.contains("mentions")));
    }
}

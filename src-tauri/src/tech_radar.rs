// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Technology Radar — Computed personal tech radar from existing 4DA data
//!
//! Synthesizes a ThoughtWorks-style Technology Radar from domain profile,
//! developer decisions, topic affinities, and source item mentions.
//! This is a computed view — nothing is stored.
//!
//! Computation engine lives in `tech_radar_compute`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Result;

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
pub async fn get_tech_radar() -> Result<TechRadar> {
    let conn = crate::open_db_connection()?;
    compute_radar(&conn)
}

#[tauri::command]
pub async fn get_radar_entry(name: String) -> Result<Option<RadarEntry>> {
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

    /// The REAL schema (every migration), not a hand-built subset. The radar
    /// reads dependencies through `build_domain_profile`, which uses the
    /// git-scoped `temporal::get_all_dependencies`; its queries touch
    /// git_signals, user_dependencies and the project_dependencies scope
    /// columns, and a subset schema made that reader error silently so the
    /// radar lost every dependency (v29 merge-queue failure, 2026-09-05).
    fn setup_test_db() -> crate::db::Database {
        let db = crate::test_utils::test_db();
        // detected_tech lives in the ACE schema and tech_stack /
        // explicit_interests in the context engine's; production applies both
        // on this same connection at app setup.
        crate::ace::db::migrate(&db.conn).expect("ACE schema");
        crate::context_engine::ContextEngine::new(db.conn.clone()).expect("context-engine schema");
        db
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
        let db = setup_test_db();
        let conn = db.conn.lock();
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
        let db = setup_test_db();
        let conn = db.conn.lock();
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
        // 0.4 + 0.2 + 0.1 = 0.7 (the engagement term was removed in v20b)
        let mut eb = EntryBuilder::new(RadarRing::Adopt, 1.0);
        eb.trend = 1.0;
        eb.decision_boost = 1.0;
        assert!((eb.score() - 0.7).abs() < 1e-6);
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
        let db = setup_test_db();
        {
            let conn = db.conn.lock();
            conn.execute("INSERT INTO tech_stack (technology) VALUES ('rust')", [])
                .unwrap();
        }
        // Real rows through the real helper (content_hash, embedding, FTS):
        // the production schema rejects the bare INSERT the old subset accepted.
        for i in 0..8 {
            crate::test_utils::insert_test_item(
                &db,
                "hackernews",
                &format!("hn-{i}"),
                &format!("Rust {i} release notes"),
                "Rust programming language news",
            );
        }

        let conn = db.conn.lock();
        let radar = compute_radar(&conn).unwrap();
        let rust = radar.entries.iter().find(|e| e.name == "rust").unwrap();
        assert_eq!(rust.movement, RadarMovement::Up);
        assert!(rust.signals.iter().any(|s| s.contains("mentions")));
    }
}

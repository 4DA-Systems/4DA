//! Context Engine - Personalized Relevance System for 4DA
//!
//! The Context Engine implements a 3-layer "Context Membrane" that transforms
//! generic information filtering into deeply personalized relevance.
//!
//! ## Layer Architecture
//!
//! 1. **Static Identity** (Phase 1) - Explicit user-declared interests
//!    - Role, tech stack, domains
//!    - Explicit topic interests with embeddings
//!    - Exclusion list (topics to never show)
//!
//! 2. **Active Context** (Phase 2) - Real-time awareness
//!    - Watched directories
//!    - Recent file modifications
//!    - Git commit analysis
//!    - Project detection
//!
//! 3. **Learned Behavior** (Phase 3) - Implicit preferences
//!    - Click/save/dismiss tracking
//!    - Topic affinities
//!    - Temporal decay
//!
//! All context data stays LOCAL - privacy is core to 4DA.

use parking_lot::Mutex;
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// ============================================================================
// Types - Static Identity (Layer 1)
// ============================================================================

/// User's static identity - explicit declarations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StaticIdentity {
    /// Professional role (e.g., "Backend Developer", "Data Scientist")
    pub role: Option<String>,

    /// Tech stack the user works with
    pub tech_stack: Vec<String>,

    /// Domains of interest (e.g., "distributed systems", "ML infrastructure")
    pub domains: Vec<String>,

    /// Explicit topic interests
    pub interests: Vec<Interest>,

    /// Topics to exclude (never show content about these)
    pub exclusions: Vec<String>,
}

/// A single interest with optional embedding for semantic matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interest {
    pub id: Option<i64>,
    pub topic: String,
    pub weight: f32, // 1.0 for explicit, lower for inferred
    pub embedding: Option<Vec<f32>>,
    pub source: InterestSource,
}

/// Where an interest came from
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum InterestSource {
    #[default]
    Explicit, // User directly added
    GitHub,   // Imported from GitHub
    Import,   // Imported from other sources
    Inferred, // System inferred from behavior
}

// ============================================================================
// Types - Interaction Tracking
// ============================================================================
// Note: ActiveContext, WatchedDirectory, TopicWeight, LearnedBehavior,
// TopicAffinity, and Interaction structs were removed (2026-01-21) as they
// are unused - ACE module provides the active implementations.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InteractionType {
    Click,   // Opened the item
    Save,    // Bookmarked
    Dismiss, // Marked not relevant
    Ignore,  // Scrolled past
}

// ============================================================================
// Context Membrane - Removed (2026-01-21)
// ============================================================================
// The ContextMembrane struct, its methods (compute_relevance, compute_static_score,
// compute_active_score, compute_learned_score, compute_anti_penalty), and the
// cosine_similarity function were removed as they are unused.
// ACE module provides the active unified scoring implementation.

// ============================================================================
// Context Engine - Database-backed context management
// ============================================================================

pub struct ContextEngine {
    conn: Arc<Mutex<Connection>>,
}

impl ContextEngine {
    /// Create a new context engine using an existing database connection
    pub fn new(conn: Arc<Mutex<Connection>>) -> SqliteResult<Self> {
        let engine = Self { conn };
        engine.migrate()?;
        Ok(engine)
    }

    /// Run database migrations for context engine tables
    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock();

        conn.execute_batch("
            -- User identity and role
            CREATE TABLE IF NOT EXISTS user_identity (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                role TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );

            -- Tech stack
            CREATE TABLE IF NOT EXISTS tech_stack (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                technology TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT (datetime('now'))
            );

            -- Domains of interest
            CREATE TABLE IF NOT EXISTS domains (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                domain TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT (datetime('now'))
            );

            -- Explicit interests with embeddings
            CREATE TABLE IF NOT EXISTS explicit_interests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                weight REAL DEFAULT 1.0,
                embedding BLOB,
                source TEXT DEFAULT 'explicit',
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_interests_topic ON explicit_interests(topic);

            -- Exclusions (topics to never show)
            CREATE TABLE IF NOT EXISTS exclusions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_exclusions_topic ON exclusions(topic);

            -- Watched directories (Phase 2)
            CREATE TABLE IF NOT EXISTS watched_directories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                enabled INTEGER DEFAULT 1,
                last_indexed TEXT,
                chunk_count INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );

            -- Interactions for learning (Phase 3 + ACE compatible)
            CREATE TABLE IF NOT EXISTS interactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER,
                item_id INTEGER,
                action TEXT,
                action_type TEXT,
                action_data TEXT,
                item_topics TEXT,
                item_source TEXT,
                signal_strength REAL DEFAULT 0.5,
                timestamp TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_interactions_item ON interactions(source_item_id);
            CREATE INDEX IF NOT EXISTS idx_interactions_action ON interactions(action);
            CREATE INDEX IF NOT EXISTS idx_interactions_source ON interactions(item_source);
            CREATE INDEX IF NOT EXISTS idx_interactions_timestamp ON interactions(timestamp);
            CREATE INDEX IF NOT EXISTS idx_interactions_item_action ON interactions(item_id, action_type);

            -- Learned topic affinities (Phase 3 + ACE compatible)
            CREATE TABLE IF NOT EXISTS topic_affinities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                embedding BLOB,
                positive_signals INTEGER DEFAULT 0,
                negative_signals INTEGER DEFAULT 0,
                total_exposures INTEGER DEFAULT 0,
                affinity_score REAL DEFAULT 0.0,
                confidence REAL DEFAULT 0.0,
                last_interaction TEXT DEFAULT (datetime('now')),
                decay_applied INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_affinities_topic ON topic_affinities(topic);
            CREATE INDEX IF NOT EXISTS idx_topic_affinities_score ON topic_affinities(affinity_score);
            CREATE INDEX IF NOT EXISTS idx_topic_affinities_last_interaction ON topic_affinities(last_interaction);

            -- Initialize singleton user identity if not exists
            INSERT OR IGNORE INTO user_identity (id) VALUES (1);
        ")?;

        info!(target: "4da::context", "Context engine tables initialized");
        Ok(())
    }

    // ========================================================================
    // Static Identity Operations (Layer 1)
    // ========================================================================

    /// Get user role
    pub fn get_role(&self) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row("SELECT role FROM user_identity WHERE id = 1", [], |row| {
            row.get(0)
        })
    }

    /// Set user role
    pub fn set_role(&self, role: Option<&str>) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE user_identity SET role = ?1, updated_at = datetime('now') WHERE id = 1",
            params![role],
        )?;
        Ok(())
    }

    /// Get tech stack
    pub fn get_tech_stack(&self) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT technology FROM tech_stack ORDER BY technology")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    /// Add technology to stack
    pub fn add_technology(&self, tech: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO tech_stack (technology) VALUES (?1)",
            params![tech],
        )?;
        Ok(())
    }

    /// Remove technology from stack
    pub fn remove_technology(&self, tech: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM tech_stack WHERE technology = ?1",
            params![tech],
        )?;
        Ok(())
    }

    /// Get domains
    pub fn get_domains(&self) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT domain FROM domains ORDER BY domain")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    /// Add domain
    #[allow(dead_code)] // Reason: domain management API, not yet exposed via settings UI
    pub fn add_domain(&self, domain: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO domains (domain) VALUES (?1)",
            params![domain],
        )?;
        Ok(())
    }

    /// Remove domain
    #[allow(dead_code)] // Reason: domain management API, not yet exposed via settings UI
    pub fn remove_domain(&self, domain: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM domains WHERE domain = ?1", params![domain])?;
        Ok(())
    }

    /// Get all explicit interests
    pub fn get_interests(&self) -> SqliteResult<Vec<Interest>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT id, topic, weight, embedding, source FROM explicit_interests")?;

        let rows = stmt.query_map([], |row| {
            let embedding_blob: Option<Vec<u8>> = row.get(3)?;
            let source_str: String = row.get(4)?;
            let source = match source_str.as_str() {
                "github" => InterestSource::GitHub,
                "import" => InterestSource::Import,
                "inferred" => InterestSource::Inferred,
                _ => InterestSource::Explicit,
            };

            Ok(Interest {
                id: Some(row.get(0)?),
                topic: row.get(1)?,
                weight: row.get(2)?,
                embedding: embedding_blob.map(|blob| blob_to_embedding(&blob)),
                source,
            })
        })?;

        rows.collect()
    }

    /// Add an interest
    pub fn add_interest(
        &self,
        topic: &str,
        weight: f32,
        embedding: Option<&[f32]>,
        source: InterestSource,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        let embedding_blob = embedding.map(embedding_to_blob);
        let source_str = match source {
            InterestSource::Explicit => "explicit",
            InterestSource::GitHub => "github",
            InterestSource::Import => "import",
            InterestSource::Inferred => "inferred",
        };

        conn.execute(
            "INSERT INTO explicit_interests (topic, weight, embedding, source)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(topic) DO UPDATE SET
                weight = excluded.weight,
                embedding = excluded.embedding,
                source = excluded.source",
            params![topic, weight, embedding_blob, source_str],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Remove an interest
    pub fn remove_interest(&self, topic: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM explicit_interests WHERE topic = ?1",
            params![topic],
        )?;
        Ok(())
    }

    /// Get all exclusions
    pub fn get_exclusions(&self) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT topic FROM exclusions ORDER BY topic")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    /// Add an exclusion
    pub fn add_exclusion(&self, topic: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO exclusions (topic) VALUES (?1)",
            params![topic],
        )?;
        Ok(())
    }

    /// Remove an exclusion
    pub fn remove_exclusion(&self, topic: &str) -> SqliteResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM exclusions WHERE topic = ?1", params![topic])?;
        Ok(())
    }

    /// Get the complete static identity
    pub fn get_static_identity(&self) -> SqliteResult<StaticIdentity> {
        Ok(StaticIdentity {
            role: self.get_role()?,
            tech_stack: self.get_tech_stack()?,
            domains: self.get_domains()?,
            interests: self.get_interests()?,
            exclusions: self.get_exclusions()?,
        })
    }

    // ========================================================================
    // Interaction Tracking (Layer 3 - Foundation)
    // ========================================================================

    /// Record an interaction
    pub fn record_interaction(
        &self,
        source_item_id: i64,
        action: InteractionType,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock();
        let action_str = match action {
            InteractionType::Click => "click",
            InteractionType::Save => "save",
            InteractionType::Dismiss => "dismiss",
            InteractionType::Ignore => "ignore",
        };

        conn.execute(
            "INSERT INTO interactions (source_item_id, item_id, action, action_type, signal_strength) VALUES (?1, ?1, ?2, ?2, 0.5)",
            params![source_item_id, action_str],
        )?;
        Ok(())
    }

    /// Get interaction counts for an item
    #[allow(dead_code)] // Reason: analytics API, not yet exposed via commands
    pub fn get_interaction_counts(
        &self,
        source_item_id: i64,
    ) -> SqliteResult<HashMap<String, u32>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT action, COUNT(*) FROM interactions WHERE source_item_id = ?1 GROUP BY action",
        )?;

        let mut counts = HashMap::new();
        let rows = stmt.query_map(params![source_item_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;

        for row in rows {
            let (action, count) = row?;
            counts.insert(action, count);
        }

        Ok(counts)
    }

    // Note: get_context_membrane was removed (2026-01-21) as ContextMembrane
    // struct was removed. Use get_static_identity() or ACE for context needs.

    /// Get interest count
    pub fn interest_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM explicit_interests", [], |row| {
            row.get(0)
        })
    }

    /// Get exclusion count
    pub fn exclusion_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM exclusions", [], |row| row.get(0))
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert f32 embedding to blob for storage
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert blob back to f32 embedding
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0u8; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::Arc;

    /// Create an in-memory ContextEngine for testing.
    fn test_engine() -> ContextEngine {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        ContextEngine::new(Arc::new(Mutex::new(conn))).expect("context engine init")
    }

    #[test]
    fn test_embedding_conversion() {
        let original = vec![1.0, 2.5, -0.5, 0.0];
        let blob = embedding_to_blob(&original);
        let restored = blob_to_embedding(&blob);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_embedding_conversion_empty() {
        let original: Vec<f32> = vec![];
        let blob = embedding_to_blob(&original);
        let restored = blob_to_embedding(&blob);
        assert!(
            restored.is_empty(),
            "Empty embedding should round-trip to empty"
        );
    }

    #[test]
    fn test_embedding_conversion_384_dim() {
        let original: Vec<f32> = (0..384).map(|i| (i as f32) * 0.001).collect();
        let blob = embedding_to_blob(&original);
        let restored = blob_to_embedding(&blob);
        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!(
                (a - b).abs() < f32::EPSILON,
                "Mismatch at value: {a} vs {b}"
            );
        }
    }

    // ========================================================================
    // Interest merge: Explicit should win over Inferred for same topic
    // ========================================================================

    /// When the same topic is added as both Inferred and Explicit, the last
    /// write wins due to UPSERT (ON CONFLICT DO UPDATE). So adding Explicit
    /// AFTER Inferred should result in the interest being Explicit.
    /// This is the correct behavior: explicit user declarations override inferences.
    #[test]
    fn test_interest_merge_prefers_explicit() {
        let engine = test_engine();

        // Add "rust" as Inferred with weight 0.5
        engine
            .add_interest("rust", 0.5, None, InterestSource::Inferred)
            .expect("add inferred interest");

        // Verify it's Inferred
        let interests = engine.get_interests().expect("get interests");
        assert_eq!(interests.len(), 1);
        assert_eq!(interests[0].source, InterestSource::Inferred);
        assert!((interests[0].weight - 0.5).abs() < f32::EPSILON);

        // Add "rust" as Explicit with weight 1.0 — should UPSERT and override
        engine
            .add_interest("rust", 1.0, None, InterestSource::Explicit)
            .expect("add explicit interest");

        // Verify it's now Explicit with updated weight
        let interests = engine.get_interests().expect("get interests after merge");
        assert_eq!(
            interests.len(),
            1,
            "UPSERT should not create a duplicate — got {} interests",
            interests.len()
        );
        assert_eq!(
            interests[0].source,
            InterestSource::Explicit,
            "Explicit should override Inferred after UPSERT"
        );
        assert!(
            (interests[0].weight - 1.0).abs() < f32::EPSILON,
            "Weight should be updated to 1.0, got {}",
            interests[0].weight
        );
    }

    /// The reverse case: adding Inferred AFTER Explicit should also replace,
    /// but in practice the system should avoid this (Explicit always wins).
    /// This test documents the current UPSERT behavior: last-write-wins.
    #[test]
    fn test_interest_merge_last_write_wins() {
        let engine = test_engine();

        engine
            .add_interest("typescript", 1.0, None, InterestSource::Explicit)
            .expect("add explicit");

        engine
            .add_interest("typescript", 0.3, None, InterestSource::Inferred)
            .expect("add inferred");

        let interests = engine.get_interests().expect("get interests");
        assert_eq!(
            interests.len(),
            1,
            "Should still be one interest after UPSERT"
        );
        // Last write wins — this is the current behavior
        assert_eq!(
            interests[0].source,
            InterestSource::Inferred,
            "Last-write-wins: Inferred overwrites Explicit (caller must enforce ordering)"
        );
        assert!(
            (interests[0].weight - 0.3).abs() < f32::EPSILON,
            "Weight should be 0.3 from the second insert, got {}",
            interests[0].weight
        );
    }

    // ========================================================================
    // Interest weight bounds: weights from DB should be in valid range
    // ========================================================================

    /// Weights added via add_interest should be preserved exactly.
    /// The system trusts callers to provide valid weights, but we verify
    /// that round-tripping through the DB doesn't corrupt values.
    #[test]
    fn test_interest_weight_bounds() {
        let engine = test_engine();

        // Add interests at boundary weights
        engine
            .add_interest("min_weight", 0.0, None, InterestSource::Explicit)
            .expect("add 0.0 weight");
        engine
            .add_interest("max_weight", 1.0, None, InterestSource::Explicit)
            .expect("add 1.0 weight");
        engine
            .add_interest("mid_weight", 0.5, None, InterestSource::Inferred)
            .expect("add 0.5 weight");

        let interests = engine.get_interests().expect("get interests");
        assert_eq!(interests.len(), 3);

        for interest in &interests {
            assert!(
                interest.weight >= 0.0 && interest.weight <= 1.0,
                "Interest '{}' weight {} should be in [0.0, 1.0]",
                interest.topic,
                interest.weight
            );
        }

        // Verify specific values round-tripped correctly
        let min = interests.iter().find(|i| i.topic == "min_weight").unwrap();
        assert!(
            (min.weight - 0.0).abs() < f32::EPSILON,
            "0.0 weight should round-trip exactly, got {}",
            min.weight
        );

        let max = interests.iter().find(|i| i.topic == "max_weight").unwrap();
        assert!(
            (max.weight - 1.0).abs() < f32::EPSILON,
            "1.0 weight should round-trip exactly, got {}",
            max.weight
        );
    }

    // ========================================================================
    // InterestSource serialization
    // ========================================================================

    /// Verify all InterestSource variants survive the DB round-trip.
    #[test]
    fn test_interest_source_round_trip_all_variants() {
        let engine = test_engine();

        let sources = vec![
            ("explicit_topic", InterestSource::Explicit),
            ("github_topic", InterestSource::GitHub),
            ("import_topic", InterestSource::Import),
            ("inferred_topic", InterestSource::Inferred),
        ];

        for (topic, source) in &sources {
            engine
                .add_interest(topic, 0.8, None, source.clone())
                .expect(&format!("add {topic}"));
        }

        let interests = engine.get_interests().expect("get interests");
        assert_eq!(interests.len(), 4);

        for (topic, expected_source) in &sources {
            let interest = interests
                .iter()
                .find(|i| i.topic == *topic)
                .unwrap_or_else(|| panic!("Missing interest: {topic}"));
            assert_eq!(
                &interest.source, expected_source,
                "Source for '{topic}' should be {expected_source:?}, got {:?}",
                interest.source
            );
        }
    }

    // ========================================================================
    // Embedding round-trip through add_interest / get_interests
    // ========================================================================

    /// Verify that embeddings stored via add_interest are correctly
    /// retrieved via get_interests (blob encoding round-trip).
    #[test]
    fn test_interest_embedding_round_trip() {
        let engine = test_engine();

        let embedding: Vec<f32> = (0..384).map(|i| (i as f32) * 0.002 - 0.384).collect();
        engine
            .add_interest("rust", 1.0, Some(&embedding), InterestSource::Explicit)
            .expect("add interest with embedding");

        let interests = engine.get_interests().expect("get interests");
        assert_eq!(interests.len(), 1);

        let stored = interests[0]
            .embedding
            .as_ref()
            .expect("embedding should be present");
        assert_eq!(stored.len(), 384, "Embedding dimension should be 384");

        for (i, (a, b)) in embedding.iter().zip(stored.iter()).enumerate() {
            assert!(
                (a - b).abs() < f32::EPSILON,
                "Mismatch at index {i}: {a} vs {b}"
            );
        }
    }

    /// Interest added without embedding should have None embedding.
    #[test]
    fn test_interest_no_embedding_returns_none() {
        let engine = test_engine();

        engine
            .add_interest("python", 0.7, None, InterestSource::Explicit)
            .expect("add interest without embedding");

        let interests = engine.get_interests().expect("get interests");
        assert_eq!(interests.len(), 1);
        assert!(
            interests[0].embedding.is_none(),
            "Interest without embedding should have None, got Some({} dims)",
            interests[0].embedding.as_ref().map_or(0, |e| e.len())
        );
    }

    // ========================================================================
    // Exclusion management
    // ========================================================================

    #[test]
    fn test_exclusion_add_and_remove() {
        let engine = test_engine();

        engine.add_exclusion("crypto").expect("add exclusion");
        engine.add_exclusion("blockchain").expect("add exclusion");

        let exclusions = engine.get_exclusions().expect("get exclusions");
        assert_eq!(exclusions.len(), 2);
        assert!(exclusions.contains(&"blockchain".to_string()));
        assert!(exclusions.contains(&"crypto".to_string()));

        engine.remove_exclusion("crypto").expect("remove exclusion");
        let exclusions = engine.get_exclusions().expect("get exclusions");
        assert_eq!(exclusions.len(), 1);
        assert_eq!(exclusions[0], "blockchain");
    }

    #[test]
    fn test_exclusion_duplicate_is_ignored() {
        let engine = test_engine();

        engine.add_exclusion("spam").expect("add first");
        engine.add_exclusion("spam").expect("add duplicate");

        let count = engine.exclusion_count().expect("count");
        assert_eq!(
            count, 1,
            "Duplicate exclusion should be ignored via INSERT OR IGNORE"
        );
    }

    // ========================================================================
    // Static identity aggregate
    // ========================================================================

    #[test]
    fn test_static_identity_aggregates_all_layers() {
        let engine = test_engine();

        engine
            .set_role(Some("Backend Developer"))
            .expect("set role");
        engine.add_technology("rust").expect("add tech");
        engine.add_technology("typescript").expect("add tech");
        engine
            .add_interest("distributed systems", 1.0, None, InterestSource::Explicit)
            .expect("add interest");
        engine.add_exclusion("crypto").expect("add exclusion");

        let identity = engine.get_static_identity().expect("get identity");
        assert_eq!(identity.role.as_deref(), Some("Backend Developer"));
        assert_eq!(identity.tech_stack.len(), 2);
        assert_eq!(identity.interests.len(), 1);
        assert_eq!(identity.interests[0].topic, "distributed systems");
        assert_eq!(identity.exclusions.len(), 1);
        assert_eq!(identity.exclusions[0], "crypto");
    }

    // Note: test_cosine_similarity and test_exclusion_filter were removed
    // as they tested the removed ContextMembrane functionality.
    // ACE module provides comprehensive relevance scoring tests.
}

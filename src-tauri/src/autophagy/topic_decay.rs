//! Topic decay analysis — computes per-topic engagement half-lives.
//!
//! By bucketing engagement by content age, we discover how quickly each
//! source type's content loses value. Security content stays relevant longer
//! (168h half-life); trending/hype decays faster (24h).

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::error::{Result, ResultExt};

/// Default half-life in hours when insufficient data exists.
const DEFAULT_HALF_LIFE_HOURS: f32 = 72.0;
/// Default peak relevance age in hours.
const DEFAULT_PEAK_HOURS: f32 = 6.0;

/// Age buckets for engagement analysis (in hours).
const BUCKET_YOUNG: f32 = 24.0; // 0-24h
const BUCKET_MID: f32 = 72.0; // 24-72h
                              // 72h+ = old

/// Analyze topic decay: compute per-source_type half-lives based on engagement patterns.
///
/// Joins source_items with feedback, buckets engagement by content age at the time
/// of feedback, and derives decay characteristics per source_type.
pub(crate) fn analyze_topic_decay(conn: &Connection) -> Vec<super::TopicDecayProfile> {
    // Query: for each feedback event, compute the age of the content at feedback time.
    // Group by source_type and age bucket.
    let mut stmt = match conn.prepare(
        "SELECT si.source_type,
                CAST((julianday(f.created_at) - julianday(si.created_at)) * 24 AS REAL) AS age_hours,
                f.relevant
         FROM feedback f
         JOIN source_items si ON f.source_item_id = si.id
         WHERE f.relevant = 1
           AND si.created_at IS NOT NULL
           AND f.created_at IS NOT NULL",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Topic decay query failed");
            return vec![];
        }
    };

    // Per source_type: count engagement in each age bucket
    // (young_count, mid_count, old_count)
    let mut buckets: HashMap<String, (i64, i64, i64)> = HashMap::new();

    let rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Topic decay row iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (source_type, age_hours, _relevant) = row;
        let entry = buckets.entry(source_type).or_insert((0, 0, 0));
        if age_hours < BUCKET_YOUNG as f64 {
            entry.0 += 1;
        } else if age_hours < BUCKET_MID as f64 {
            entry.1 += 1;
        } else {
            entry.2 += 1;
        }
    }

    if buckets.is_empty() {
        debug!(target: "4da::autophagy", "No engagement data for topic decay analysis");
        return vec![];
    }

    let mut profiles = Vec::new();

    for (source_type, (young, mid, old)) in &buckets {
        let total = young + mid + old;
        if total == 0 {
            continue;
        }

        let (half_life_hours, peak_hours) = compute_decay_params(*young, *mid, *old);

        profiles.push(super::TopicDecayProfile {
            topic: source_type.clone(),
            half_life_hours,
            peak_relevance_age_hours: peak_hours,
        });
    }

    info!(
        target: "4da::autophagy",
        profiles = profiles.len(),
        "Topic decay analysis complete"
    );

    profiles
}

/// Compute decay parameters from engagement bucket counts.
///
/// The half-life is the age at which engagement drops to 50% of peak.
/// We use a simple heuristic based on where engagement concentrates.
fn compute_decay_params(young: i64, mid: i64, old: i64) -> (f32, f32) {
    let total = (young + mid + old) as f32;
    if total == 0.0 {
        return (DEFAULT_HALF_LIFE_HOURS, DEFAULT_PEAK_HOURS);
    }

    let young_ratio = young as f32 / total;
    let mid_ratio = mid as f32 / total;
    let old_ratio = old as f32 / total;

    // Determine peak relevance age
    let peak_hours = if young_ratio >= mid_ratio && young_ratio >= old_ratio {
        // Engagement concentrated in young bucket -> fast-decay content
        DEFAULT_PEAK_HOURS
    } else if mid_ratio >= old_ratio {
        // Engagement peaks in mid-range
        36.0
    } else {
        // Long-lived content (security, research)
        72.0
    };

    // Estimate half-life from distribution shape
    let half_life = if young_ratio > 0.6 {
        // >60% engagement in first 24h -> short half-life (trending/hype)
        24.0
    } else if young_ratio + mid_ratio > 0.8 {
        // Most engagement within 72h -> medium half-life
        DEFAULT_HALF_LIFE_HOURS
    } else if old_ratio > 0.3 {
        // Significant engagement after 72h -> long half-life (security, research)
        168.0
    } else {
        DEFAULT_HALF_LIFE_HOURS
    };

    (half_life, peak_hours)
}

/// Store decay profiles to `digested_intelligence`, superseding previous entries.
pub(crate) fn store_decay_profiles(
    conn: &Connection,
    profiles: &[super::TopicDecayProfile],
) -> Result<()> {
    for profile in profiles {
        let data = serde_json::to_string(&serde_json::json!({
            "half_life_hours": profile.half_life_hours,
            "peak_relevance_age_hours": profile.peak_relevance_age_hours,
        }))?;

        // Insert new decay profile first, then point old rows at it.
        // This order satisfies the FK constraint on superseded_by -> digested_intelligence(id).
        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('topic_decay', ?1, ?2, 0.8, 0)",
            params![profile.topic, data],
        )
        .with_context(|| format!("Failed to insert decay profile for {}", profile.topic))?;

        let new_id = conn.last_insert_rowid();

        // Supersede previous decay profiles for this topic (excluding the one just inserted)
        conn.execute(
            "UPDATE digested_intelligence
             SET superseded_by = ?1
             WHERE digest_type = 'topic_decay' AND subject = ?2 AND superseded_by IS NULL AND id != ?1",
            params![new_id, profile.topic],
        )
        .with_context(|| format!("Failed to supersede decay profile for {}", profile.topic))?;
    }

    debug!(target: "4da::autophagy", count = profiles.len(), "Stored topic decay profiles");
    Ok(())
}

/// Load topic decay profiles for the scoring pipeline.
///
/// Returns a map of topic -> half_life_hours. Topics not in the map should use
/// the default half-life of 72 hours.
pub(crate) fn load_topic_decay_profiles(conn: &Connection) -> HashMap<String, f32> {
    let mut result = HashMap::new();

    let mut stmt = match conn.prepare(
        "SELECT subject, data FROM digested_intelligence
         WHERE digest_type = 'topic_decay' AND superseded_by IS NULL
         ORDER BY created_at DESC",
    ) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Failed to load topic decay profiles");
            return result;
        }
    };

    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Failed to iterate decay profile rows");
            return result;
        }
    };

    for row in rows.flatten() {
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&row.1) {
            if let Some(hl) = data
                .get("half_life_hours")
                .and_then(serde_json::Value::as_f64)
            {
                result.insert(row.0, hl as f32);
            }
        }
    }

    debug!(target: "4da::autophagy", count = result.len(), "Loaded topic decay profiles");
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL DEFAULT '',
                url TEXT,
                title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL DEFAULT '',
                embedding BLOB,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                summary TEXT,
                embedding_status TEXT DEFAULT 'pending',
                embed_text TEXT
            );
            CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );
            CREATE TABLE digested_intelligence (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                digest_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.5,
                sample_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT,
                superseded_by INTEGER
            );",
        )
        .expect("create tables");
        conn
    }

    #[test]
    fn test_analyze_topic_decay_empty() {
        let conn = setup_test_db();
        let profiles = analyze_topic_decay(&conn);
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_compute_decay_params_young_dominated() {
        // 80% engagement in young bucket -> short half-life
        let (half_life, peak) = compute_decay_params(80, 15, 5);
        assert_eq!(half_life, 24.0);
        assert_eq!(peak, DEFAULT_PEAK_HOURS);
    }

    #[test]
    fn test_compute_decay_params_old_dominated() {
        // Significant old engagement -> long half-life (security/research)
        let (half_life, _peak) = compute_decay_params(20, 30, 50);
        assert_eq!(half_life, 168.0);
    }

    #[test]
    fn test_store_and_load_decay_profiles() {
        let conn = setup_test_db();

        let profiles = vec![
            super::super::TopicDecayProfile {
                topic: "hackernews".to_string(),
                half_life_hours: 24.0,
                peak_relevance_age_hours: 6.0,
            },
            super::super::TopicDecayProfile {
                topic: "arxiv".to_string(),
                half_life_hours: 168.0,
                peak_relevance_age_hours: 72.0,
            },
        ];

        store_decay_profiles(&conn, &profiles).expect("store");

        let loaded = load_topic_decay_profiles(&conn);
        assert_eq!(loaded.len(), 2);
        assert!((loaded["hackernews"] - 24.0).abs() < 0.01);
        assert!((loaded["arxiv"] - 168.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_decay_params_zero_data() {
        let (half_life, peak) = compute_decay_params(0, 0, 0);
        assert_eq!(half_life, DEFAULT_HALF_LIFE_HOURS);
        assert_eq!(peak, DEFAULT_PEAK_HOURS);
    }
}

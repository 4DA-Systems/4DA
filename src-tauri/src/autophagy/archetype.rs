// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Dismissal archetype detection — clusters dismissed items into recurring patterns.
//!
//! Unlike anti-patterns (which detect per-source biases), archetypes detect
//! per-topic+source+content_type dismissal patterns. When users consistently
//! dismiss items matching a specific archetype, future items of that archetype
//! get penalized in the scoring pipeline.
//!
//! Inspired by TitanCA's PairVul domain adaptation: rather than learning from
//! individual items, we learn from structural *archetypes* — recurring combinations
//! of topic, source, and content type that the user rejects.

use std::collections::HashMap;

use crate::error::{Result, ResultExt};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Stop words excluded from topic keyword extraction.
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "in", "on", "at", "to", "for", "of", "and", "or", "but", "not", "with",
    "this", "that", "from", "by", "as", "it", "its", "are", "was", "be", "has", "had", "have",
    "new", "how", "why", "what", "when", "you", "your",
];

/// Minimum sample size to declare a valid archetype (avoids false positives).
const MIN_SAMPLE_SIZE: u32 = 8;

/// Minimum dismissal rate to qualify as a negative archetype.
const MIN_DISMISSAL_RATE: f32 = 0.70;

/// Maximum penalty that can be suggested for a single archetype.
const MAX_PENALTY: f32 = 0.25;

/// A detected dismissal archetype — a recurring pattern of dismissed items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DismissalArchetype {
    /// Composite key: "topic:source_type:content_type" (content_type may be "unknown")
    pub archetype_id: String,
    /// Human-readable description
    pub description: String,
    /// Topic component (extracted from title keywords)
    pub topic: String,
    /// Source type component
    pub source_type: String,
    /// Content type component (security_advisory, release_notes, discussion, etc.)
    pub content_type: String,
    /// Fraction of items in this archetype that were dismissed (0.0-1.0)
    pub dismissal_rate: f32,
    /// Total items seen matching this archetype
    pub sample_size: u32,
    /// Suggested scoring penalty (0.0-0.25, proportional to dismissal rate)
    pub suggested_penalty: f32,
}

/// Check whether the `content_type` column exists on `source_items`.
///
/// Older databases created before Phase 55 migration lack this column.
/// We fall back to "unknown" for all items when the column is absent.
fn has_content_type_column(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('source_items') WHERE name = 'content_type'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

/// Extract meaningful topic keywords from a title.
///
/// Filters to lowercase words > 3 chars, removes stop words, keeps at most 5.
fn extract_keywords(title: &str) -> Vec<String> {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .filter(|w| !STOP_WORDS.contains(w))
        .take(5)
        .map(std::string::ToString::to_string)
        .collect()
}

/// Intermediate accumulator for per-archetype counts.
#[derive(Default)]
struct ArchetypeStats {
    dismissed: u32,
    total: u32,
}

/// Detect dismissal archetypes from the last `max_age_days` of feedback data.
///
/// Groups dismissed items by (primary_topic, source_type, content_type) and identifies
/// combinations with dismissal rates >= 70% on at least 8 samples. These represent
/// structural patterns the user consistently rejects.
pub(crate) fn detect_archetypes(conn: &Connection, max_age_days: i64) -> Vec<DismissalArchetype> {
    let has_ct = has_content_type_column(conn);
    let window = format!("-{max_age_days} days");

    // Build the query dynamically based on column availability
    let ct_expr = if has_ct {
        "COALESCE(si.content_type, 'unknown')"
    } else {
        "'unknown'"
    };

    let query = format!(
        "SELECT si.source_type, si.title, {ct_expr}, f.relevant
         FROM source_items si
         INNER JOIN feedback f ON f.source_item_id = si.id
         WHERE f.created_at >= datetime('now', ?1)"
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(s) => s,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Archetype detection query failed");
            return vec![];
        }
    };

    // Accumulate stats per (keyword, source_type, content_type)
    let mut stats: HashMap<(String, String, String), ArchetypeStats> = HashMap::new();

    let rows = match stmt.query_map(params![window], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    }) {
        Ok(r) => r,
        Err(e) => {
            warn!(target: "4da::autophagy", error = %e, "Archetype detection row iteration failed");
            return vec![];
        }
    };

    for row in rows.flatten() {
        let (source_type, title, content_type, relevant) = row;
        let keywords = extract_keywords(&title);

        for keyword in keywords {
            let key = (keyword, source_type.clone(), content_type.clone());
            let entry = stats.entry(key).or_default();
            entry.total += 1;
            if relevant == 0 {
                entry.dismissed += 1;
            }
        }
    }

    // Filter to high-signal archetypes
    let mut archetypes = Vec::new();
    for ((topic, source_type, content_type), counts) in &stats {
        if counts.total < MIN_SAMPLE_SIZE {
            continue;
        }

        let dismissal_rate = counts.dismissed as f32 / counts.total as f32;
        if dismissal_rate < MIN_DISMISSAL_RATE {
            continue;
        }

        let suggested_penalty = ((dismissal_rate - 0.5) * 0.5).min(MAX_PENALTY);
        let archetype_id = format!("{topic}:{source_type}:{content_type}");
        let description = format!(
            "Items about {topic} from {source_type} ({content_type}) dismissed {:.0}% of the time",
            dismissal_rate * 100.0
        );

        archetypes.push(DismissalArchetype {
            archetype_id,
            description,
            topic: topic.clone(),
            source_type: source_type.clone(),
            content_type: content_type.clone(),
            dismissal_rate,
            sample_size: counts.total,
            suggested_penalty,
        });
    }

    info!(
        target: "4da::autophagy",
        archetypes_detected = archetypes.len(),
        total_groups = stats.len(),
        "Dismissal archetype detection complete"
    );

    archetypes
}

/// Store detected archetypes to `digested_intelligence`, superseding previous entries.
///
/// Each archetype is stored with `digest_type = 'dismissal_archetype'` and subject
/// set to the archetype_id. Previous non-superseded entries for the same subject
/// are pointed at the new row via `superseded_by`.
pub(crate) fn store_archetypes(conn: &Connection, archetypes: &[DismissalArchetype]) -> Result<()> {
    for archetype in archetypes {
        let data = serde_json::to_string(&serde_json::json!({
            "topic": archetype.topic,
            "source_type": archetype.source_type,
            "content_type": archetype.content_type,
            "dismissal_rate": archetype.dismissal_rate,
            "sample_size": archetype.sample_size,
            "suggested_penalty": archetype.suggested_penalty,
            "description": archetype.description,
        }))?;

        let confidence = (archetype.sample_size as f32 / 20.0).min(1.0);

        conn.execute(
            "INSERT INTO digested_intelligence (digest_type, subject, data, confidence, sample_size)
             VALUES ('dismissal_archetype', ?1, ?2, ?3, ?4)",
            params![
                archetype.archetype_id,
                data,
                confidence,
                archetype.sample_size,
            ],
        )
        .with_context(|| {
            format!(
                "Failed to insert dismissal archetype for {}",
                archetype.archetype_id
            )
        })?;

        let new_id = conn.last_insert_rowid();

        // Supersede previous entries for the same archetype
        conn.execute(
            "UPDATE digested_intelligence
             SET superseded_by = ?1
             WHERE digest_type = 'dismissal_archetype'
               AND subject = ?2
               AND superseded_by IS NULL
               AND id != ?1",
            params![new_id, archetype.archetype_id],
        )
        .with_context(|| {
            format!(
                "Failed to supersede dismissal archetype for {}",
                archetype.archetype_id
            )
        })?;
    }

    debug!(target: "4da::autophagy", count = archetypes.len(), "Stored dismissal archetypes");
    Ok(())
}

/// Load current archetype penalties from `digested_intelligence`.
///
/// Returns a map of archetype_id -> suggested_penalty for all non-superseded
/// dismissal archetypes. Returns an empty map if no data exists or on query failure.
pub(crate) fn load_archetype_penalties(conn: &Connection) -> HashMap<String, f32> {
    let mut result = HashMap::new();

    let Ok(mut stmt) = conn.prepare(
        "SELECT subject, data FROM digested_intelligence
         WHERE digest_type = 'dismissal_archetype' AND superseded_by IS NULL",
    ) else {
        return result;
    };

    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return result;
    };

    for row in rows.flatten() {
        let (subject, data) = row;
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(penalty) = parsed.get("suggested_penalty").and_then(|v| v.as_f64()) {
                result.insert(subject, penalty as f32);
            }
        }
    }

    result
}

/// Compute the archetype penalty for a specific item.
///
/// Given loaded archetype penalties and an item's attributes, finds the maximum
/// matching penalty. Matches are checked for each title keyword against both
/// the exact content_type and the "unknown" fallback.
///
/// Returns the single highest penalty found (not a sum, to avoid over-penalizing
/// items that match multiple archetypes).
pub(crate) fn archetype_penalty_for_item(
    penalties: &HashMap<String, f32>,
    source_type: &str,
    title: &str,
    content_type: Option<&str>,
) -> f32 {
    if penalties.is_empty() {
        return 0.0;
    }

    let keywords = extract_keywords(title);
    let ct = content_type.unwrap_or("unknown");
    let mut max_penalty: f32 = 0.0;

    for keyword in &keywords {
        // Check exact match: keyword:source_type:content_type
        let exact_key = format!("{keyword}:{source_type}:{ct}");
        if let Some(&penalty) = penalties.get(&exact_key) {
            max_penalty = max_penalty.max(penalty);
        }

        // Fallback: keyword:source_type:unknown (catches items without content_type)
        if ct != "unknown" {
            let fallback_key = format!("{keyword}:{source_type}:unknown");
            if let Some(&penalty) = penalties.get(&fallback_key) {
                max_penalty = max_penalty.max(penalty);
            }
        }
    }

    max_penalty
}

#[cfg(test)]
#[path = "archetype_tests.rs"]
mod tests;

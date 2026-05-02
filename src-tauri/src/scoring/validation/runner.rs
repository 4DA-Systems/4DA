// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Validation Runner — Scores real DB items against 10 personas
//!
//! For each persona, scores the last 500 fetched items, takes the top 20,
//! and auto-judges relevance via topic/title overlap. Produces a
//! `ValidationReport` with precision@20, separation scores, and recommendations.

use serde::Serialize;
use tracing::info;

use crate::db::Database;
use crate::error::{Result, ResultExt};
use crate::scoring::{self, ScoringInput, ScoringOptions};

use super::personas::{all_validation_contexts, SimulatedPersona};

// ============================================================================
// Report Types
// ============================================================================

/// Per-persona validation result.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub persona: String,
    pub precision_at_20: f32,
    pub anti_topic_leaks: usize,
    pub avg_score_relevant: f32,
    pub avg_score_irrelevant: f32,
    pub separation: f32,
    pub top_20_titles: Vec<String>,
    pub items_scored: usize,
}

/// Aggregate validation report across all personas.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub timestamp: String,
    pub personas: Vec<ValidationResult>,
    pub overall_precision: f32,
    pub worst_persona: String,
    pub best_persona: String,
    pub separation_score: f32,
    pub recommendations: Vec<String>,
    pub total_items_in_db: usize,
}

// ============================================================================
// Topic Matching (auto-judge)
// ============================================================================

/// Check if an item's title or content contains any of the expected topics.
fn is_relevant_to_persona(title: &str, content: &str, expected_topics: &[&str]) -> bool {
    let text = format!(
        "{} {}",
        title.to_lowercase(),
        content.to_lowercase().chars().take(500).collect::<String>()
    );
    expected_topics
        .iter()
        .any(|topic| text.contains(&topic.to_lowercase()))
}

/// Check if an item's title or content contains any anti-topics.
fn has_anti_topic_leak(
    title: &str,
    content: &str,
    anti_topics: &[&str],
    expected_topics: &[&str],
) -> bool {
    let text = format!(
        "{} {}",
        title.to_lowercase(),
        content.to_lowercase().chars().take(500).collect::<String>()
    );
    // Only count as a leak if the item matches an anti-topic
    // but does NOT match any expected topic (mixed-domain items are OK)
    let has_anti = anti_topics
        .iter()
        .any(|topic| text.contains(&topic.to_lowercase()));
    let has_expected = expected_topics
        .iter()
        .any(|topic| text.contains(&topic.to_lowercase()));
    has_anti && !has_expected
}

// ============================================================================
// Validation Runner
// ============================================================================

/// A scored content item: (id, title, content, source_type, embedding, created_at).
type ScoredItem = (
    i64,
    String,
    String,
    String,
    Vec<f32>,
    Option<chrono::DateTime<chrono::Utc>>,
);

/// Score a persona against a set of items, returning the validation result.
fn validate_persona(
    persona: &SimulatedPersona,
    ctx: &scoring::ScoringContext,
    items: &[ScoredItem],
    db: &Database,
) -> ValidationResult {
    let options = ScoringOptions {
        apply_freshness: false,
        apply_signals: false,
        trend_topics: vec![],
    };

    // Score all items
    let mut scored: Vec<(f32, &str, &str, bool)> = items
        .iter()
        .map(|(id, title, content, source_type, embedding, created_at)| {
            let input = ScoringInput {
                id: *id as u64,
                title,
                url: None,
                content,
                source_type,
                embedding,
                created_at: created_at.as_ref(),
                detected_lang: "en",
                source_tags: &[],
                tags_json: None,
            };
            let result = scoring::score_item(&input, ctx, db, &options, None);
            (
                result.top_score,
                title.as_str(),
                content.as_str(),
                result.relevant,
            )
        })
        .collect();

    // Sort descending by score
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Take top 20 for precision@20
    let top_20: Vec<_> = scored.iter().take(20).collect();
    let top_20_titles: Vec<String> = top_20.iter().map(|(_, t, _, _)| t.to_string()).collect();

    // Auto-judge: how many of the top 20 are relevant to this persona?
    let relevant_in_top20 = top_20
        .iter()
        .filter(|(_, title, content, _)| {
            is_relevant_to_persona(title, content, &persona.expected_topics)
        })
        .count();
    let precision_at_20 = if top_20.is_empty() {
        0.0
    } else {
        relevant_in_top20 as f32 / top_20.len() as f32
    };

    // Count anti-topic leaks in top 20
    let anti_topic_leaks = top_20
        .iter()
        .filter(|(_, title, content, _)| {
            has_anti_topic_leak(
                title,
                content,
                &persona.anti_topics,
                &persona.expected_topics,
            )
        })
        .count();

    // Compute average scores for relevant vs irrelevant items (across ALL scored items)
    let mut relevant_scores = Vec::new();
    let mut irrelevant_scores = Vec::new();
    for (score, title, content, _) in &scored {
        if is_relevant_to_persona(title, content, &persona.expected_topics) {
            relevant_scores.push(*score);
        } else {
            irrelevant_scores.push(*score);
        }
    }

    let avg_score_relevant = if relevant_scores.is_empty() {
        0.0
    } else {
        relevant_scores.iter().sum::<f32>() / relevant_scores.len() as f32
    };
    let avg_score_irrelevant = if irrelevant_scores.is_empty() {
        0.0
    } else {
        irrelevant_scores.iter().sum::<f32>() / irrelevant_scores.len() as f32
    };
    let separation = avg_score_relevant - avg_score_irrelevant;

    ValidationResult {
        persona: persona.name.to_string(),
        precision_at_20,
        anti_topic_leaks,
        avg_score_relevant,
        avg_score_irrelevant,
        separation,
        top_20_titles,
        items_scored: scored.len(),
    }
}

/// Run the full scoring validation across all 10 personas.
///
/// Loads up to 500 recent items from the database and scores them
/// against each persona's ScoringContext. Returns a comprehensive
/// `ValidationReport` with per-persona metrics and recommendations.
#[tauri::command]
pub async fn run_scoring_validation() -> Result<ValidationReport> {
    let db = crate::get_database()?;
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Load recent items (last 7 days, up to 500)
    let stored_items = db
        .get_items_since_hours(168, 500)
        .context("Failed to load items for validation")?;

    let total_items_in_db = stored_items.len();

    if total_items_in_db == 0 {
        return Ok(ValidationReport {
            timestamp,
            personas: vec![],
            overall_precision: 0.0,
            worst_persona: "N/A".to_string(),
            best_persona: "N/A".to_string(),
            separation_score: 0.0,
            recommendations: vec!["No items in database. Run a fetch cycle first.".to_string()],
            total_items_in_db: 0,
        });
    }

    info!(
        target: "4da::validation",
        items = total_items_in_db,
        "Starting scoring validation across 10 personas"
    );

    // Pre-extract item data to avoid holding DB lock during scoring
    let items: Vec<_> = stored_items
        .iter()
        .map(|item| {
            (
                item.id,
                item.title.clone(),
                item.content.clone(),
                item.source_type.clone(),
                item.embedding.clone(),
                Some(item.created_at),
            )
        })
        .collect();

    // Build all persona contexts and run validation
    let persona_contexts = all_validation_contexts();
    let mut results: Vec<ValidationResult> = Vec::with_capacity(persona_contexts.len());

    for (persona, ctx) in &persona_contexts {
        let result = validate_persona(persona, ctx, &items, db);
        info!(
            target: "4da::validation",
            persona = persona.name,
            precision = format!("{:.1}%", result.precision_at_20 * 100.0),
            anti_leaks = result.anti_topic_leaks,
            separation = format!("{:.4}", result.separation),
            "Persona validation complete"
        );
        results.push(result);
    }

    // Aggregate metrics
    let overall_precision = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.precision_at_20).sum::<f32>() / results.len() as f32
    };

    let separation_score = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.separation).sum::<f32>() / results.len() as f32
    };

    let best = results
        .iter()
        .max_by(|a, b| {
            a.precision_at_20
                .partial_cmp(&b.precision_at_20)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or_else(|| "N/A".to_string(), |r| r.persona.clone());

    let worst = results
        .iter()
        .min_by(|a, b| {
            a.precision_at_20
                .partial_cmp(&b.precision_at_20)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or_else(|| "N/A".to_string(), |r| r.persona.clone());

    // Generate recommendations
    let recommendations = generate_recommendations(&results, overall_precision, separation_score);

    info!(
        target: "4da::validation",
        overall_precision = format!("{:.1}%", overall_precision * 100.0),
        best_persona = &best,
        worst_persona = &worst,
        separation = format!("{:.4}", separation_score),
        "Scoring validation complete"
    );

    Ok(ValidationReport {
        timestamp,
        personas: results,
        overall_precision,
        worst_persona: worst,
        best_persona: best,
        separation_score,
        recommendations,
        total_items_in_db,
    })
}

// ============================================================================
// Recommendations Engine
// ============================================================================

fn generate_recommendations(
    results: &[ValidationResult],
    overall_precision: f32,
    separation_score: f32,
) -> Vec<String> {
    let mut recs = Vec::new();

    // Overall precision check
    if overall_precision < 0.3 {
        recs.push(
            "Overall precision below 30% — scoring pipeline may need fundamental tuning."
                .to_string(),
        );
    } else if overall_precision < 0.5 {
        recs.push(
            "Overall precision between 30-50% — keyword and interest matching needs improvement."
                .to_string(),
        );
    } else if overall_precision >= 0.7 {
        recs.push("Overall precision above 70% — scoring engine performing well.".to_string());
    }

    // Separation check
    if separation_score < 0.02 {
        recs.push(
            "Separation score near zero — relevant and irrelevant items score similarly. \
             Consider strengthening interest and keyword signals."
                .to_string(),
        );
    } else if separation_score < 0.05 {
        recs.push(
            "Separation score below 0.05 — marginal discrimination between relevant and noise."
                .to_string(),
        );
    }

    // Per-persona issues
    let low_precision_personas: Vec<_> = results
        .iter()
        .filter(|r| r.precision_at_20 < 0.2 && r.items_scored > 0)
        .collect();
    if !low_precision_personas.is_empty() {
        let names: Vec<_> = low_precision_personas
            .iter()
            .map(|r| r.persona.as_str())
            .collect();
        recs.push(format!(
            "Low precision (<20%) for: {}. These personas may lack \
             sufficient keyword coverage in their domains.",
            names.join(", ")
        ));
    }

    // Anti-topic leaks
    let leaky_personas: Vec<_> = results.iter().filter(|r| r.anti_topic_leaks > 3).collect();
    if !leaky_personas.is_empty() {
        let names: Vec<_> = leaky_personas
            .iter()
            .map(|r| format!("{} ({})", r.persona, r.anti_topic_leaks))
            .collect();
        recs.push(format!(
            "Anti-topic leaks detected: {}. Exclusion filters may need strengthening.",
            names.join(", ")
        ));
    }

    // Negative separation (scoring inversions)
    let inverted: Vec<_> = results.iter().filter(|r| r.separation < -0.01).collect();
    if !inverted.is_empty() {
        let names: Vec<_> = inverted.iter().map(|r| r.persona.as_str()).collect();
        recs.push(format!(
            "Score inversion (irrelevant > relevant) for: {}. Pipeline may be \
             boosting wrong signals for these domains.",
            names.join(", ")
        ));
    }

    if recs.is_empty() {
        recs.push("All validation checks passed. Scoring engine is healthy.".to_string());
    }

    recs
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_is_relevant_to_persona() {
        let expected = vec!["rust", "tokio", "async"];
        assert!(is_relevant_to_persona(
            "Rust 1.75 Released",
            "New features in Rust async",
            &expected
        ));
        assert!(!is_relevant_to_persona(
            "React 19 Released",
            "New hooks in React",
            &expected
        ));
    }

    #[test]
    fn test_anti_topic_leak_detection() {
        let anti = vec!["cryptocurrency", "nft"];
        let expected = vec!["rust", "async"];

        // Pure anti-topic item = leak
        assert!(has_anti_topic_leak(
            "NFT Market Crashes",
            "cryptocurrency prices fall",
            &anti,
            &expected
        ));

        // Mixed item (has both expected and anti) = not a leak
        assert!(!has_anti_topic_leak(
            "Rust NFT Library Released",
            "A new rust library for nft",
            &anti,
            &expected
        ));

        // Neither expected nor anti = not a leak
        assert!(!has_anti_topic_leak(
            "Python 3.12 Released",
            "New Python features",
            &anti,
            &expected
        ));
    }

    #[test]
    fn test_generate_recommendations_all_healthy() {
        let results = vec![ValidationResult {
            persona: "Test".to_string(),
            precision_at_20: 0.8,
            anti_topic_leaks: 0,
            avg_score_relevant: 0.5,
            avg_score_irrelevant: 0.2,
            separation: 0.3,
            top_20_titles: vec![],
            items_scored: 100,
        }];
        let recs = generate_recommendations(&results, 0.8, 0.3);
        assert!(recs.iter().any(|r| r.contains("performing well")));
    }

    #[test]
    fn test_generate_recommendations_low_precision() {
        let results = vec![ValidationResult {
            persona: "Weak".to_string(),
            precision_at_20: 0.1,
            anti_topic_leaks: 5,
            avg_score_relevant: 0.3,
            avg_score_irrelevant: 0.35,
            separation: -0.05,
            top_20_titles: vec![],
            items_scored: 100,
        }];
        let recs = generate_recommendations(&results, 0.1, -0.05);
        assert!(recs.iter().any(|r| r.contains("fundamental tuning")));
        assert!(recs.iter().any(|r| r.contains("Anti-topic leaks")));
        assert!(recs.iter().any(|r| r.contains("Score inversion")));
    }

    #[test]
    fn test_empty_report() {
        let recs = generate_recommendations(&[], 0.0, 0.0);
        // With 0.0 overall precision, should flag it
        assert!(recs.iter().any(|r| r.contains("fundamental tuning")));
    }

    #[test]
    fn test_all_personas_build_contexts() {
        // Verify all 10 personas can build ScoringContexts without panicking
        let contexts = all_validation_contexts();
        assert_eq!(contexts.len(), 10);
        for (persona, ctx) in &contexts {
            assert!(!persona.name.is_empty());
            assert!(!persona.expected_topics.is_empty());
            assert!(!persona.anti_topics.is_empty());
            assert!(ctx.interest_count > 0);
            assert!(!ctx.interests.is_empty());
            assert!(!ctx.declared_tech.is_empty());
        }
    }

    #[test]
    fn test_persona_expected_topics_distinct() {
        // Verify that personas have meaningfully different expected topics
        let personas = super::super::personas::all_validation_personas();
        let mut topic_sets: Vec<HashSet<&str>> = Vec::new();

        for persona in &personas {
            let set: HashSet<&str> = persona.expected_topics.iter().copied().collect();
            // Each persona should have unique topics relative to others
            for (i, other_set) in topic_sets.iter().enumerate() {
                let overlap: usize = set.intersection(other_set).count();
                let max_overlap_ratio = overlap as f32 / set.len().min(other_set.len()) as f32;
                // Allow some overlap but not complete (< 80%)
                assert!(
                    max_overlap_ratio < 0.8,
                    "Personas {} and {} have {:.0}% topic overlap — too similar",
                    personas[i].name,
                    persona.name,
                    max_overlap_ratio * 100.0
                );
            }
            topic_sets.push(set);
        }
    }
}

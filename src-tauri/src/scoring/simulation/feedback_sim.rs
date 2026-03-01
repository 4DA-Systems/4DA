//! Feedback Simulation — deterministic session-based feedback engine
//!
//! Simulates user feedback over multiple sessions without DB writes.
//! ScoringContext cannot be cloned, so we use factory functions to rebuild
//! contexts with updated feedback boosts each session.

use std::collections::HashMap;

use super::super::ace_context::ACEContext;
use super::super::{score_item, ScoringContext};
use super::corpus::corpus;
use super::{sim_db, sim_input, sim_no_freshness};
use super::{ContentCategory, ExpectedOutcome, LabeledItem};

// ============================================================================
// Types
// ============================================================================

pub(super) struct FeedbackEvent {
    #[allow(dead_code)]
    pub item_id: u64,
    pub topic: String,
    #[allow(dead_code)]
    pub relevant: bool,
    pub delta: f64,
}

// ============================================================================
// Persona context factories with feedback boosts
// ============================================================================

/// Build a Rust systems dev context with the given feedback boosts
pub(super) fn rust_ctx_with_boosts(
    boosts: &HashMap<String, f64>,
    interaction_count: i64,
) -> ScoringContext {
    let emb = vec![0.5_f32; 384];
    let make_interest = |id: i64, topic: &str, weight: f32| crate::context_engine::Interest {
        id: Some(id),
        topic: topic.to_string(),
        weight,
        embedding: Some(emb.clone()),
        source: crate::context_engine::InterestSource::Explicit,
    };

    let interests = vec![
        make_interest(1, "Rust", 1.0),
        make_interest(2, "systems programming", 1.0),
        make_interest(3, "Tauri", 0.9),
        make_interest(4, "SQLite", 0.8),
        make_interest(5, "WebAssembly", 0.7),
    ];

    let mut ace = ACEContext::default();
    ace.active_topics.push("rust".to_string());
    ace.active_topics.push("tauri".to_string());
    ace.active_topics.push("sqlite".to_string());
    ace.detected_tech.push("rust".to_string());
    ace.detected_tech.push("tauri".to_string());
    ace.detected_tech.push("sqlite".to_string());

    let domain = crate::domain_profile::DomainProfile {
        primary_stack: ["rust", "tauri", "sqlite"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        adjacent_tech: ["tokio", "serde", "wasm", "typescript"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        all_tech: [
            "rust",
            "tauri",
            "sqlite",
            "tokio",
            "serde",
            "wasm",
            "typescript",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        dependency_names: ["tokio", "serde", "sqlx", "tauri"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        interest_topics: ["rust", "systems programming", "tauri", "sqlite"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        domain_concerns: std::collections::HashSet::new(),
    };

    let stack = crate::stacks::compose_profiles(&["rust_systems".to_string()]);

    ScoringContext::builder()
        .interest_count(5)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "rust".to_string(),
            "tauri".to_string(),
            "sqlite".to_string(),
        ])
        .composed_stack(stack)
        .feedback_boosts(boosts.clone())
        .feedback_interaction_count(interaction_count)
        .build()
}

/// Build a Python ML context with the given feedback boosts
pub(super) fn python_ctx_with_boosts(
    boosts: &HashMap<String, f64>,
    interaction_count: i64,
) -> ScoringContext {
    let emb = vec![0.5_f32; 384];
    let make_interest = |id: i64, topic: &str, weight: f32| crate::context_engine::Interest {
        id: Some(id),
        topic: topic.to_string(),
        weight,
        embedding: Some(emb.clone()),
        source: crate::context_engine::InterestSource::Explicit,
    };

    let interests = vec![
        make_interest(1, "Machine Learning", 1.0),
        make_interest(2, "Python", 1.0),
        make_interest(3, "LLM", 0.9),
        make_interest(4, "PyTorch", 0.9),
        make_interest(5, "data science", 0.7),
    ];

    let mut ace = ACEContext::default();
    ace.active_topics.push("python".to_string());
    ace.active_topics.push("pytorch".to_string());
    ace.active_topics.push("machine learning".to_string());
    ace.detected_tech.push("python".to_string());
    ace.detected_tech.push("pytorch".to_string());

    let domain = crate::domain_profile::DomainProfile {
        primary_stack: ["python", "pytorch", "tensorflow"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        adjacent_tech: ["numpy", "pandas", "scikit-learn", "huggingface"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        all_tech: [
            "python",
            "pytorch",
            "tensorflow",
            "numpy",
            "pandas",
            "scikit-learn",
            "huggingface",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        dependency_names: ["torch", "transformers", "numpy", "pandas"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        interest_topics: ["machine learning", "python", "llm", "pytorch"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        domain_concerns: std::collections::HashSet::new(),
    };

    let stack = crate::stacks::compose_profiles(&["python_ml".to_string()]);

    ScoringContext::builder()
        .interest_count(5)
        .interests(interests)
        .ace_ctx(ace)
        .domain_profile(domain)
        .declared_tech(vec![
            "python".to_string(),
            "pytorch".to_string(),
            "tensorflow".to_string(),
        ])
        .composed_stack(stack)
        .feedback_boosts(boosts.clone())
        .feedback_interaction_count(interaction_count)
        .build()
}

// ============================================================================
// Session simulation
// ============================================================================

/// Simulate one feedback session: score items, generate feedback events
pub(super) fn simulate_session(
    ctx: &ScoringContext,
    items: &[LabeledItem],
    persona_idx: usize,
) -> Vec<FeedbackEvent> {
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let mut events = Vec::new();
    for item in items {
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result = score_item(&input, ctx, &db, &opts, None);
        let expected = item.expected[persona_idx];

        match expected {
            ExpectedOutcome::StrongRelevant => {
                // User clicks on relevant items
                if result.relevant {
                    events.push(FeedbackEvent {
                        item_id: item.id,
                        topic: derive_topic(&item.category),
                        relevant: true,
                        delta: 0.15,
                    });
                }
            }
            ExpectedOutcome::NotRelevant => {
                // User dismisses noise
                if result.relevant {
                    events.push(FeedbackEvent {
                        item_id: item.id,
                        topic: derive_topic(&item.category),
                        relevant: false,
                        delta: -0.10,
                    });
                }
            }
            _ => {}
        }
    }
    events
}

/// Apply feedback events to build new topic boost map (with 0.95 decay)
pub(super) fn apply_feedback(
    existing: &HashMap<String, f64>,
    events: &[FeedbackEvent],
) -> HashMap<String, f64> {
    let mut boosts: HashMap<String, f64> = existing
        .iter()
        .map(|(k, v)| (k.clone(), v * 0.95))
        .collect();

    for event in events {
        let entry = boosts.entry(event.topic.clone()).or_insert(0.0);
        *entry = (*entry + event.delta).clamp(-1.0, 1.0);
    }
    boosts
}

/// Returns labeled items that represent realistic lifecycle content
pub(super) fn lifecycle_corpus() -> Vec<LabeledItem> {
    corpus()
        .into_iter()
        .filter(|item| {
            matches!(
                item.category,
                ContentCategory::DirectMatch
                    | ContentCategory::AdjacentMatch
                    | ContentCategory::CrossDomainNoise
                    | ContentCategory::Borderline
            )
        })
        .collect()
}

/// Score all corpus items against a context, return mean score of relevant items
#[allow(dead_code)]
pub(super) fn score_corpus_against_ctx(ctx: &ScoringContext, persona_idx: usize) -> f64 {
    let items = lifecycle_corpus();
    let db = sim_db();
    let opts = sim_no_freshness();
    let emb = vec![0.0_f32; 384];

    let mut relevant_scores = Vec::new();
    for item in &items {
        let expected = item.expected[persona_idx];
        if !matches!(
            expected,
            ExpectedOutcome::StrongRelevant | ExpectedOutcome::WeakRelevant
        ) {
            continue;
        }
        let input = sim_input(item.id, item.title, item.content, &emb);
        let result = score_item(&input, ctx, &db, &opts, None);
        relevant_scores.push(result.top_score as f64);
    }

    if relevant_scores.is_empty() {
        return 0.0;
    }
    relevant_scores.iter().sum::<f64>() / relevant_scores.len() as f64
}

// ============================================================================
// Helpers
// ============================================================================

fn derive_topic(category: &ContentCategory) -> String {
    match category {
        ContentCategory::DirectMatch => "core_tech".to_string(),
        ContentCategory::AdjacentMatch => "adjacent_tech".to_string(),
        ContentCategory::CrossDomainNoise => "cross_domain".to_string(),
        ContentCategory::SecurityAdvisory => "security".to_string(),
        ContentCategory::ReleaseNotes => "releases".to_string(),
        _ => "general".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_apply_feedback_accumulates() {
        let existing = HashMap::new();
        let events = vec![
            FeedbackEvent { item_id: 1, topic: "core_tech".to_string(), relevant: true, delta: 0.15 },
            FeedbackEvent { item_id: 2, topic: "core_tech".to_string(), relevant: true, delta: 0.15 },
        ];
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        assert!((val - 0.30).abs() < 0.01, "Expected ~0.30, got {:.4}", val);
    }

    #[test]
    fn test_apply_feedback_decay() {
        let mut existing = HashMap::new();
        existing.insert("core_tech".to_string(), 0.50);
        let events: Vec<FeedbackEvent> = vec![];
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        assert!((val - 0.475).abs() < 0.01, "Expected ~0.475 after decay, got {:.4}", val);
    }

    #[test]
    fn test_apply_feedback_clamps() {
        let existing = HashMap::new();
        let events: Vec<FeedbackEvent> = (0..20)
            .map(|i| FeedbackEvent {
                item_id: i,
                topic: "core_tech".to_string(),
                relevant: true,
                delta: 0.15,
            })
            .collect();
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        assert!((val - 1.0).abs() < f64::EPSILON, "Expected clamped to 1.0, got {:.4}", val);
    }

    #[test]
    fn test_apply_feedback_mixed() {
        let existing = HashMap::new();
        let events = vec![
            FeedbackEvent { item_id: 1, topic: "core_tech".to_string(), relevant: true, delta: 0.15 },
            FeedbackEvent { item_id: 2, topic: "core_tech".to_string(), relevant: true, delta: 0.15 },
            FeedbackEvent { item_id: 3, topic: "core_tech".to_string(), relevant: false, delta: -0.10 },
        ];
        let result = apply_feedback(&existing, &events);
        let val = result.get("core_tech").copied().unwrap_or(0.0);
        // 0 + 0.15 + 0.15 - 0.10 = 0.20
        assert!((val - 0.20).abs() < 0.01, "Expected ~0.20, got {:.4}", val);
    }

    #[test]
    fn test_simulate_session_returns_events() {
        let boosts = HashMap::new();
        let ctx = rust_ctx_with_boosts(&boosts, 50);
        let items = corpus();
        let events = simulate_session(&ctx, &items, 0);
        // With 215 corpus items, Rust persona should generate at least some feedback events
        assert!(!events.is_empty(), "simulate_session should produce at least one feedback event");
    }

    #[test]
    fn test_rust_ctx_factory() {
        let boosts = HashMap::new();
        let ctx = rust_ctx_with_boosts(&boosts, 50);
        assert_eq!(ctx.interest_count, 5, "Rust ctx should have 5 interests");
        assert_eq!(ctx.interests.len(), 5);
        assert_eq!(ctx.feedback_interaction_count, 50);
    }

    #[test]
    fn test_python_ctx_factory() {
        let boosts = HashMap::new();
        let ctx = python_ctx_with_boosts(&boosts, 40);
        assert_eq!(ctx.interest_count, 5, "Python ctx should have 5 interests");
        assert_eq!(ctx.interests.len(), 5);
        assert_eq!(ctx.feedback_interaction_count, 40);
    }

    #[test]
    fn test_lifecycle_corpus_smaller_than_full() {
        let full = corpus();
        let lifecycle = lifecycle_corpus();
        assert!(
            lifecycle.len() < full.len(),
            "lifecycle_corpus ({}) should be smaller than full corpus ({})",
            lifecycle.len(),
            full.len()
        );
        assert!(!lifecycle.is_empty(), "lifecycle_corpus should not be empty");
    }
}

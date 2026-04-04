// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! AWE Synthesis Bridge — connects 4DA's behavioral data to AWE's wisdom pipeline.
//!
//! This module queries 4DA's rich behavioral tables (topic_affinities, digested_intelligence,
//! decision_windows, interactions, feedback, advantage_score) and feeds structured context
//! into AWE's transmutation pipeline for personalized wisdom synthesis.
//!
//! Two integration paths:
//! 1. **DeveloperContext JSON** — written to temp file, passed to AWE CLI via --context_file
//! 2. **Direct LLM synthesis** — builds behavioral summary and calls LLM directly for wisdom

use serde::Serialize;
use tracing::{info, warn};

type Result<T> = std::result::Result<T, String>;

// ============================================================================
// Behavioral Context Types
// ============================================================================

/// Aggregated behavioral context from 4DA's database tables.
/// This is the bridge data structure — everything AWE needs to produce personalized wisdom.
#[derive(Debug, Clone, Serialize)]
pub struct BehavioralContext {
    /// Top topic affinities with scores (from topic_affinities table)
    pub topic_affinities: Vec<TopicSignal>,
    /// Recent calibration insights (from digested_intelligence)
    pub calibration_insights: Vec<CalibrationInsight>,
    /// Decision window outcomes (from decision_windows)
    pub decision_outcomes: Vec<DecisionOutcome>,
    /// Interaction patterns — engagement velocity and source preferences
    pub interaction_patterns: InteractionPatterns,
    /// Compound advantage trajectory (from advantage_score)
    pub advantage_trajectory: Vec<AdvantagePoint>,
    /// Feedback coverage stats
    pub feedback_stats: FeedbackStats,
    /// Tech stack detected by ACE
    pub detected_tech: Vec<String>,
    /// Active working topics
    pub active_topics: Vec<String>,
    /// Instant context from ACE + source_items (always populated, even cold start)
    pub instant_context: InstantContext,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopicSignal {
    pub topic: String,
    pub affinity_score: f64,
    pub confidence: f64,
    pub positive_signals: i64,
    pub negative_signals: i64,
    pub total_exposures: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalibrationInsight {
    pub digest_type: String,
    pub subject: String,
    pub data: String,
    pub confidence: f64,
    pub sample_size: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionOutcome {
    pub title: String,
    pub window_type: String,
    pub status: String,
    pub outcome: Option<String>,
    pub urgency: f64,
    pub relevance: f64,
    pub lead_time_hours: Option<f64>,
    pub opened_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InteractionPatterns {
    pub total_interactions: i64,
    pub saves: i64,
    pub dismissals: i64,
    pub clicks: i64,
    pub avg_signal_strength: f64,
    /// Top sources by engagement count
    pub top_sources: Vec<(String, i64)>,
    /// Interactions in last 7 days vs prior 7 days (growth ratio)
    pub weekly_velocity: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdvantagePoint {
    pub period: String,
    pub score: f64,
    pub windows_acted: i64,
    pub windows_expired: i64,
    pub calibration_accuracy: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackStats {
    pub total_items: i64,
    pub items_with_feedback: i64,
    pub coverage_pct: f64,
    pub positive_ratio: f64,
}

/// Instant context — seeded from ACE + source_items when behavioral data is thin.
/// Ensures Momentum is rich from day one, even before user has interacted.
#[derive(Debug, Clone, Serialize)]
pub struct InstantContext {
    /// Total items in the database (intelligence gathered)
    pub total_source_items: i64,
    /// Items from the last 24 hours
    pub items_last_24h: i64,
    /// Top source types by count
    pub source_breakdown: Vec<(String, i64)>,
    /// ACE-detected project count
    pub project_count: i64,
    /// ACE-detected dependency count
    pub dependency_count: i64,
    /// Most recent item timestamp
    pub latest_item_at: Option<String>,
    /// Data richness level: "cold" (< 10 items), "warming" (10-100), "rich" (100+)
    pub data_level: String,
}

// ============================================================================
// Build Behavioral Context (queries all 4DA tables)
// ============================================================================

/// Query all relevant 4DA behavioral data tables and assemble a BehavioralContext.
/// This is the core bridge function — everything feeds from here.
pub fn build_behavioral_context() -> Result<BehavioralContext> {
    let conn = crate::open_db_connection()
        .map_err(|e| format!("Failed to open DB: {e}"))?;

    let topic_affinities = query_topic_affinities(&conn);
    let calibration_insights = query_calibration_insights(&conn);
    let decision_outcomes = query_decision_outcomes(&conn);
    let interaction_patterns = query_interaction_patterns(&conn);
    let advantage_trajectory = query_advantage_trajectory(&conn);
    let feedback_stats = query_feedback_stats(&conn);

    let (detected_tech, active_topics) = get_ace_summary();

    Ok(BehavioralContext {
        topic_affinities,
        calibration_insights,
        decision_outcomes,
        interaction_patterns,
        advantage_trajectory,
        feedback_stats,
        detected_tech,
        active_topics,
    })
}

fn query_topic_affinities(conn: &rusqlite::Connection) -> Vec<TopicSignal> {
    conn.prepare(
        "SELECT topic, affinity_score, confidence, positive_signals, negative_signals, total_exposures
         FROM topic_affinities
         WHERE total_exposures >= 3
         ORDER BY ABS(affinity_score) DESC
         LIMIT 30",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            Ok(TopicSignal {
                topic: row.get(0)?,
                affinity_score: row.get(1)?,
                confidence: row.get(2)?,
                positive_signals: row.get(3)?,
                negative_signals: row.get(4)?,
                total_exposures: row.get(5)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

fn query_calibration_insights(conn: &rusqlite::Connection) -> Vec<CalibrationInsight> {
    conn.prepare(
        "SELECT digest_type, subject, data, confidence, sample_size, created_at
         FROM digested_intelligence
         WHERE superseded_by IS NULL
           AND (expires_at IS NULL OR expires_at > datetime('now'))
         ORDER BY confidence DESC, created_at DESC
         LIMIT 20",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            Ok(CalibrationInsight {
                digest_type: row.get(0)?,
                subject: row.get(1)?,
                data: row.get(2)?,
                confidence: row.get(3)?,
                sample_size: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

fn query_decision_outcomes(conn: &rusqlite::Connection) -> Vec<DecisionOutcome> {
    conn.prepare(
        "SELECT title, window_type, status, outcome, urgency, relevance, lead_time_hours, opened_at
         FROM decision_windows
         ORDER BY opened_at DESC
         LIMIT 30",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            Ok(DecisionOutcome {
                title: row.get(0)?,
                window_type: row.get(1)?,
                status: row.get(2)?,
                outcome: row.get(3)?,
                urgency: row.get(4)?,
                relevance: row.get(5)?,
                lead_time_hours: row.get(6)?,
                opened_at: row.get(7)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

fn query_interaction_patterns(conn: &rusqlite::Connection) -> InteractionPatterns {
    let mut patterns = InteractionPatterns {
        total_interactions: 0,
        saves: 0,
        dismissals: 0,
        clicks: 0,
        avg_signal_strength: 0.0,
        top_sources: Vec::new(),
        weekly_velocity: 1.0,
    };

    // Aggregate counts
    if let Ok(mut stmt) = conn.prepare(
        "SELECT
            COUNT(*),
            SUM(CASE WHEN action_type = 'save' THEN 1 ELSE 0 END),
            SUM(CASE WHEN action_type = 'dismiss' THEN 1 ELSE 0 END),
            SUM(CASE WHEN action_type = 'click' THEN 1 ELSE 0 END),
            AVG(COALESCE(signal_strength, 0.5))
         FROM interactions",
    ) {
        if let Ok(row) = stmt.query_row([], |row| {
            Ok((
                row.get::<_, i64>(0).unwrap_or(0),
                row.get::<_, i64>(1).unwrap_or(0),
                row.get::<_, i64>(2).unwrap_or(0),
                row.get::<_, i64>(3).unwrap_or(0),
                row.get::<_, f64>(4).unwrap_or(0.5),
            ))
        }) {
            patterns.total_interactions = row.0;
            patterns.saves = row.1;
            patterns.dismissals = row.2;
            patterns.clicks = row.3;
            patterns.avg_signal_strength = row.4;
        }
    }

    // Top sources
    if let Ok(mut stmt) = conn.prepare(
        "SELECT COALESCE(item_source, 'unknown'), COUNT(*)
         FROM interactions
         WHERE item_source IS NOT NULL
         GROUP BY item_source
         ORDER BY COUNT(*) DESC
         LIMIT 5",
    ) {
        patterns.top_sources = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                ))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
    }

    // Weekly velocity: this_week / last_week
    if let Ok(mut stmt) = conn.prepare(
        "SELECT
            (SELECT COUNT(*) FROM interactions WHERE timestamp > datetime('now', '-7 days')),
            (SELECT COUNT(*) FROM interactions WHERE timestamp BETWEEN datetime('now', '-14 days') AND datetime('now', '-7 days'))",
    ) {
        if let Ok((this_week, last_week)) = stmt.query_row([], |row| {
            Ok((
                row.get::<_, f64>(0).unwrap_or(0.0),
                row.get::<_, f64>(1).unwrap_or(0.0),
            ))
        }) {
            patterns.weekly_velocity = if last_week > 0.0 {
                this_week / last_week
            } else if this_week > 0.0 {
                2.0 // Growing from zero
            } else {
                1.0
            };
        }
    }

    patterns
}

fn query_advantage_trajectory(conn: &rusqlite::Connection) -> Vec<AdvantagePoint> {
    conn.prepare(
        "SELECT period, score, windows_acted, windows_expired, calibration_accuracy
         FROM advantage_score
         ORDER BY computed_at DESC
         LIMIT 10",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            Ok(AdvantagePoint {
                period: row.get(0)?,
                score: row.get(1)?,
                windows_acted: row.get(2)?,
                windows_expired: row.get(3)?,
                calibration_accuracy: row.get(4)?,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

fn query_feedback_stats(conn: &rusqlite::Connection) -> FeedbackStats {
    let mut stats = FeedbackStats {
        total_items: 0,
        items_with_feedback: 0,
        coverage_pct: 0.0,
        positive_ratio: 0.0,
    };

    if let Ok(mut stmt) = conn.prepare(
        "SELECT
            (SELECT COUNT(*) FROM source_items),
            (SELECT COUNT(DISTINCT source_item_id) FROM feedback),
            (SELECT CAST(SUM(CASE WHEN relevant = 1 THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(*), 0) FROM feedback)",
    ) {
        if let Ok(row) = stmt.query_row([], |row| {
            Ok((
                row.get::<_, i64>(0).unwrap_or(0),
                row.get::<_, i64>(1).unwrap_or(0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        }) {
            stats.total_items = row.0;
            stats.items_with_feedback = row.1;
            stats.coverage_pct = if row.0 > 0 {
                (row.1 as f64 / row.0 as f64) * 100.0
            } else {
                0.0
            };
            stats.positive_ratio = row.2;
        }
    }

    stats
}

/// Get ACE-detected tech stack and active topics.
fn get_ace_summary() -> (Vec<String>, Vec<String>) {
    let ace_ctx = crate::scoring::get_ace_context();
    (ace_ctx.detected_tech.clone(), ace_ctx.active_topics.clone())
}

// ============================================================================
// DeveloperContext JSON (for AWE CLI --context_file)
// ============================================================================

/// Build AWE DeveloperContext JSON from 4DA behavioral data.
/// Written to a temp file and passed to AWE CLI via --context_file.
pub fn build_developer_context_json(ctx: &BehavioralContext) -> Result<String> {
    let knowledge_gaps: Vec<String> = ctx
        .topic_affinities
        .iter()
        .filter(|t| t.affinity_score > 0.3 && t.confidence < 0.4)
        .take(10)
        .map(|t| t.topic.clone())
        .collect();

    let blind_spots: Vec<String> = ctx
        .topic_affinities
        .iter()
        .filter(|t| t.total_exposures < 5 && t.affinity_score > 0.0)
        .take(10)
        .map(|t| t.topic.clone())
        .collect();

    let decision_count = ctx.decision_outcomes.len() as u32;
    let feedback_coverage = ctx.feedback_stats.coverage_pct;

    let json = serde_json::json!({
        "primary_stack": ctx.detected_tech.iter().take(10).collect::<Vec<_>>(),
        "adjacent_tech": ctx.detected_tech.iter().skip(10).take(10).collect::<Vec<_>>(),
        "domain_concerns": extract_domain_concerns(ctx),
        "identity_summary": build_identity_summary(ctx),
        "os": std::env::consts::OS,
        "hardware_class": "desktop",
        "project_count": 1,
        "dependency_count": ctx.detected_tech.len(),
        "days_active": compute_days_active(ctx),
        "items_processed": ctx.interaction_patterns.total_interactions as u64,
        "decision_count": decision_count,
        "feedback_coverage_pct": feedback_coverage / 100.0,
        "knowledge_gaps": knowledge_gaps,
        "blind_spots": blind_spots,
    });

    serde_json::to_string_pretty(&json).map_err(|e| format!("JSON serialization failed: {e}"))
}

fn extract_domain_concerns(ctx: &BehavioralContext) -> Vec<String> {
    let mut concerns = Vec::new();
    // Infer from calibration insights
    for insight in &ctx.calibration_insights {
        if insight.digest_type == "calibration_delta" || insight.digest_type == "anti_pattern" {
            if !concerns.contains(&insight.subject) && concerns.len() < 5 {
                concerns.push(insight.subject.clone());
            }
        }
    }
    if concerns.is_empty() {
        concerns.push("software-quality".to_string());
    }
    concerns
}

fn build_identity_summary(ctx: &BehavioralContext) -> String {
    let tech = if ctx.detected_tech.is_empty() {
        "Developer".to_string()
    } else {
        let top = ctx.detected_tech.iter().take(3).cloned().collect::<Vec<_>>().join("/");
        format!("{top} developer")
    };

    let engagement = if ctx.interaction_patterns.total_interactions > 1000 {
        "highly active"
    } else if ctx.interaction_patterns.total_interactions > 100 {
        "active"
    } else {
        "building engagement"
    };

    let decisions = ctx.decision_outcomes.len();
    format!("{tech}, {engagement}, {decisions} decisions tracked")
}

fn compute_days_active(ctx: &BehavioralContext) -> u32 {
    // Estimate from earliest decision window or interaction
    if let Some(earliest) = ctx.decision_outcomes.last() {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&earliest.opened_at, "%Y-%m-%d %H:%M:%S") {
            let now = chrono::Utc::now().naive_utc();
            return (now - dt).num_days().max(1) as u32;
        }
    }
    30 // Default
}

// ============================================================================
// Wisdom Synthesis (direct LLM call)
// ============================================================================

/// Synthesize personalized daily wisdom from behavioral context.
/// This is the core AWE synthesis — produces natural language wisdom grounded in real data.
pub async fn synthesize_daily_wisdom(
    ctx: &BehavioralContext,
) -> Result<String> {
    let llm_settings = {
        let settings = crate::get_settings_manager().lock();
        settings.get().llm.clone()
    };

    if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
        return Err("No LLM configured for wisdom synthesis".into());
    }

    let behavioral_summary = format_behavioral_summary(ctx);

    let system_prompt = r#"You are AWE (Artificial Wisdom Engine), a trusted advisor who knows this developer's complete behavioral history. You speak with earned authority — you've watched their patterns, tracked their decisions, and measured their growth.

Your voice is direct, specific, and grounded in ACTUAL DATA. Never generic. Reference specific numbers, topics, and patterns from the context provided.

Rules:
- Lead with the most important insight
- Reference actual numbers: "Your 73% save rate on Rust content..."
- Name specific topics and patterns: "Your affinity for distributed-systems..."
- If decision coverage is low, note it honestly
- If engagement is growing, acknowledge momentum
- If there are blind spots, name them without judgment
- Maximum 150 words — density over length
- No markdown, no bullet points — flowing prose
- No meta-text like "Based on your data..." — just deliver the wisdom"#;

    let user_prompt = format!(
        "Developer behavioral context:\n\n{behavioral_summary}\n\n\
         Synthesize your daily wisdom. What should this developer know about themselves today?",
    );

    let llm_client = crate::llm::LLMClient::new(llm_settings);
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];

    let start = std::time::Instant::now();
    match llm_client.complete(system_prompt, messages).await {
        Ok(response) => {
            info!(
                target: "4da::awe_synthesis",
                tokens = response.input_tokens + response.output_tokens,
                elapsed_ms = start.elapsed().as_millis(),
                "Daily wisdom synthesis complete"
            );
            Ok(response.content)
        }
        Err(e) => {
            warn!(target: "4da::awe_synthesis", error = %e, "Wisdom synthesis failed");
            Err(format!("Wisdom synthesis failed: {e}"))
        }
    }
}

/// Synthesize contextual wisdom for a specific set of signals/topics.
/// Used to enrich morning briefing with behavioral context.
pub async fn synthesize_signal_context(
    topics: &[String],
    ctx: &BehavioralContext,
) -> Result<String> {
    let llm_settings = {
        let settings = crate::get_settings_manager().lock();
        settings.get().llm.clone()
    };

    if llm_settings.provider != "ollama" && llm_settings.api_key.is_empty() {
        return Err("No LLM configured".into());
    }

    // Find relevant affinities for these topics
    let relevant_affinities: Vec<&TopicSignal> = ctx
        .topic_affinities
        .iter()
        .filter(|a| {
            topics.iter().any(|t| {
                a.topic.to_lowercase().contains(&t.to_lowercase())
                    || t.to_lowercase().contains(&a.topic.to_lowercase())
            })
        })
        .collect();

    let relevant_decisions: Vec<&DecisionOutcome> = ctx
        .decision_outcomes
        .iter()
        .filter(|d| {
            topics.iter().any(|t| d.title.to_lowercase().contains(&t.to_lowercase()))
        })
        .collect();

    if relevant_affinities.is_empty() && relevant_decisions.is_empty() {
        return Ok(String::new()); // No relevant context to synthesize
    }

    let mut context_parts = Vec::new();
    if !relevant_affinities.is_empty() {
        let aff_text: Vec<String> = relevant_affinities
            .iter()
            .map(|a| {
                format!(
                    "{}: affinity {:.0}%, {} positive / {} negative signals",
                    a.topic,
                    a.affinity_score * 100.0,
                    a.positive_signals,
                    a.negative_signals,
                )
            })
            .collect();
        context_parts.push(format!("Topic history:\n{}", aff_text.join("\n")));
    }
    if !relevant_decisions.is_empty() {
        let dec_text: Vec<String> = relevant_decisions
            .iter()
            .take(5)
            .map(|d| {
                format!(
                    "[{}] {} — status: {}, outcome: {}",
                    d.window_type,
                    d.title,
                    d.status,
                    d.outcome.as_deref().unwrap_or("pending"),
                )
            })
            .collect();
        context_parts.push(format!("Related decisions:\n{}", dec_text.join("\n")));
    }

    let system_prompt = "You are AWE. Given signal topics and the developer's history with those topics, \
        produce a 1-2 sentence contextual note. Be specific — reference actual numbers and patterns. \
        If there's a relevant past decision, mention it. No markdown, no preamble.";

    let user_prompt = format!(
        "Current signals about: {}\n\n{}\n\nWhat context matters?",
        topics.join(", "),
        context_parts.join("\n\n"),
    );

    let llm_client = crate::llm::LLMClient::new(llm_settings);
    let messages = vec![crate::llm::Message {
        role: "user".to_string(),
        content: user_prompt,
    }];

    match llm_client.complete(system_prompt, messages).await {
        Ok(response) => Ok(response.content),
        Err(e) => {
            warn!(target: "4da::awe_synthesis", error = %e, "Signal context synthesis failed");
            Err(format!("Signal context failed: {e}"))
        }
    }
}

// ============================================================================
// Behavioral Summary Formatting
// ============================================================================

/// Format the behavioral context into a structured text summary for LLM consumption.
fn format_behavioral_summary(ctx: &BehavioralContext) -> String {
    let mut parts = Vec::new();

    // Tech identity
    if !ctx.detected_tech.is_empty() {
        let tech = ctx.detected_tech.iter().take(10).cloned().collect::<Vec<_>>().join(", ");
        parts.push(format!("Tech stack: {tech}"));
    }
    if !ctx.active_topics.is_empty() {
        let topics = ctx.active_topics.iter().take(8).cloned().collect::<Vec<_>>().join(", ");
        parts.push(format!("Active topics: {topics}"));
    }

    // Topic affinities (strongest signals)
    if !ctx.topic_affinities.is_empty() {
        let positive: Vec<String> = ctx
            .topic_affinities
            .iter()
            .filter(|t| t.affinity_score > 0.3)
            .take(8)
            .map(|t| format!("{} ({:.0}%, {} signals)", t.topic, t.affinity_score * 100.0, t.total_exposures))
            .collect();
        if !positive.is_empty() {
            parts.push(format!("Strong affinities: {}", positive.join(", ")));
        }

        let negative: Vec<String> = ctx
            .topic_affinities
            .iter()
            .filter(|t| t.affinity_score < -0.2)
            .take(5)
            .map(|t| format!("{} ({:.0}%)", t.topic, t.affinity_score * 100.0))
            .collect();
        if !negative.is_empty() {
            parts.push(format!("Rejected topics: {}", negative.join(", ")));
        }
    }

    // Calibration insights
    if !ctx.calibration_insights.is_empty() {
        let insights: Vec<String> = ctx
            .calibration_insights
            .iter()
            .take(5)
            .map(|c| format!("[{}] {} — confidence {:.0}% (n={})", c.digest_type, c.subject, c.confidence * 100.0, c.sample_size))
            .collect();
        parts.push(format!("Calibration insights:\n{}", insights.join("\n")));
    }

    // Decision outcomes
    let acted: Vec<&DecisionOutcome> = ctx
        .decision_outcomes
        .iter()
        .filter(|d| d.status == "acted" || d.status == "closed")
        .collect();
    let open: Vec<&DecisionOutcome> = ctx
        .decision_outcomes
        .iter()
        .filter(|d| d.status == "open")
        .collect();
    let expired: Vec<&DecisionOutcome> = ctx
        .decision_outcomes
        .iter()
        .filter(|d| d.status == "expired")
        .collect();

    if !ctx.decision_outcomes.is_empty() {
        parts.push(format!(
            "Decisions: {} total, {} acted, {} open, {} expired",
            ctx.decision_outcomes.len(),
            acted.len(),
            open.len(),
            expired.len(),
        ));
    }

    // Interaction velocity
    let ip = &ctx.interaction_patterns;
    if ip.total_interactions > 0 {
        let velocity_text = if ip.weekly_velocity > 1.5 {
            "accelerating"
        } else if ip.weekly_velocity > 0.8 {
            "steady"
        } else if ip.weekly_velocity > 0.0 {
            "declining"
        } else {
            "starting"
        };
        parts.push(format!(
            "Engagement: {} interactions ({} saves, {} dismissals), velocity: {velocity_text} ({:.1}x), avg strength: {:.2}",
            ip.total_interactions, ip.saves, ip.dismissals, ip.weekly_velocity, ip.avg_signal_strength,
        ));

        if !ip.top_sources.is_empty() {
            let src_text: Vec<String> = ip
                .top_sources
                .iter()
                .map(|(s, c)| format!("{s}: {c}"))
                .collect();
            parts.push(format!("Top sources: {}", src_text.join(", ")));
        }
    }

    // Advantage trajectory
    if !ctx.advantage_trajectory.is_empty() {
        let latest = &ctx.advantage_trajectory[0];
        parts.push(format!(
            "Compound advantage: {:.1} (acted: {}, expired: {}, calibration: {:.0}%)",
            latest.score, latest.windows_acted, latest.windows_expired, latest.calibration_accuracy * 100.0,
        ));
    }

    // Feedback coverage
    parts.push(format!(
        "Feedback: {:.1}% coverage ({} of {} items), {:.0}% positive",
        ctx.feedback_stats.coverage_pct,
        ctx.feedback_stats.items_with_feedback,
        ctx.feedback_stats.total_items,
        ctx.feedback_stats.positive_ratio * 100.0,
    ));

    parts.join("\n")
}

// ============================================================================
// AWE CLI Context File Integration
// ============================================================================

/// Write DeveloperContext JSON to a temp file for AWE CLI consumption.
/// Returns the path to the temp file.
pub fn write_context_file(ctx: &BehavioralContext) -> Result<std::path::PathBuf> {
    let json = build_developer_context_json(ctx)?;
    let dir = std::env::temp_dir().join("4da-awe");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create temp dir: {e}"))?;

    let path = dir.join("developer_context.json");
    std::fs::write(&path, &json)
        .map_err(|e| format!("Failed to write context file: {e}"))?;

    info!(
        target: "4da::awe_synthesis",
        path = %path.display(),
        bytes = json.len(),
        "Developer context file written for AWE CLI"
    );
    Ok(path)
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get the full behavioral context as JSON (for frontend display).
#[tauri::command]
pub async fn get_behavioral_context() -> Result<String> {
    let ctx = build_behavioral_context()?;
    serde_json::to_string(&ctx).map_err(|e| format!("Serialization failed: {e}"))
}

/// Synthesize daily wisdom from behavioral data.
/// Returns the LLM-generated wisdom text.
#[tauri::command]
pub async fn synthesize_wisdom() -> Result<String> {
    let ctx = build_behavioral_context()?;
    synthesize_daily_wisdom(&ctx).await
}

/// Synthesize contextual wisdom for specific topics.
#[tauri::command]
pub async fn synthesize_topic_context(topics: Vec<String>) -> Result<String> {
    let ctx = build_behavioral_context()?;
    synthesize_signal_context(&topics, &ctx).await
}

/// Write AWE developer context file and return the path.
/// Used by frontend to trigger context refresh for AWE CLI.
#[tauri::command]
pub async fn refresh_awe_context() -> Result<String> {
    let ctx = build_behavioral_context()?;
    let path = write_context_file(&ctx)?;
    Ok(path.to_string_lossy().to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_behavioral_summary_empty() {
        let ctx = BehavioralContext {
            topic_affinities: vec![],
            calibration_insights: vec![],
            decision_outcomes: vec![],
            interaction_patterns: InteractionPatterns {
                total_interactions: 0,
                saves: 0,
                dismissals: 0,
                clicks: 0,
                avg_signal_strength: 0.5,
                top_sources: vec![],
                weekly_velocity: 1.0,
            },
            advantage_trajectory: vec![],
            feedback_stats: FeedbackStats {
                total_items: 0,
                items_with_feedback: 0,
                coverage_pct: 0.0,
                positive_ratio: 0.0,
            },
            detected_tech: vec![],
            active_topics: vec![],
        };

        let summary = format_behavioral_summary(&ctx);
        assert!(summary.contains("Feedback:"));
        assert!(summary.contains("0.0% coverage"));
    }

    #[test]
    fn test_format_behavioral_summary_rich() {
        let ctx = BehavioralContext {
            topic_affinities: vec![
                TopicSignal {
                    topic: "rust".into(),
                    affinity_score: 0.85,
                    confidence: 0.92,
                    positive_signals: 47,
                    negative_signals: 3,
                    total_exposures: 50,
                },
                TopicSignal {
                    topic: "blockchain".into(),
                    affinity_score: -0.6,
                    confidence: 0.8,
                    positive_signals: 2,
                    negative_signals: 18,
                    total_exposures: 20,
                },
            ],
            calibration_insights: vec![CalibrationInsight {
                digest_type: "calibration_delta".into(),
                subject: "rust-ecosystem".into(),
                data: "improving".into(),
                confidence: 0.88,
                sample_size: 15,
                created_at: "2026-04-01 08:00:00".into(),
            }],
            decision_outcomes: vec![DecisionOutcome {
                title: "Adopt Tauri 2.0".into(),
                window_type: "technology_adoption".into(),
                status: "acted".into(),
                outcome: Some("confirmed".into()),
                urgency: 0.7,
                relevance: 0.9,
                lead_time_hours: Some(48.0),
                opened_at: "2026-03-15 10:00:00".into(),
            }],
            interaction_patterns: InteractionPatterns {
                total_interactions: 523,
                saves: 89,
                dismissals: 134,
                clicks: 300,
                avg_signal_strength: 0.62,
                top_sources: vec![("hackernews".into(), 245), ("reddit".into(), 120)],
                weekly_velocity: 1.3,
            },
            advantage_trajectory: vec![AdvantagePoint {
                period: "2026-W14".into(),
                score: 7.2,
                windows_acted: 5,
                windows_expired: 1,
                calibration_accuracy: 0.78,
            }],
            feedback_stats: FeedbackStats {
                total_items: 1200,
                items_with_feedback: 890,
                coverage_pct: 74.2,
                positive_ratio: 0.67,
            },
            detected_tech: vec!["rust".into(), "typescript".into(), "react".into()],
            active_topics: vec!["tauri".into(), "wgsl-shaders".into()],
        };

        let summary = format_behavioral_summary(&ctx);
        assert!(summary.contains("rust"));
        assert!(summary.contains("85%")); // affinity
        assert!(summary.contains("blockchain"));
        assert!(summary.contains("Rejected topics"));
        assert!(summary.contains("523 interactions"));
        assert!(summary.contains("hackernews"));
        assert!(summary.contains("74.2% coverage"));
        assert!(summary.contains("7.2")); // advantage score
    }

    #[test]
    fn test_build_developer_context_json() {
        let ctx = BehavioralContext {
            topic_affinities: vec![TopicSignal {
                topic: "rust".into(),
                affinity_score: 0.9,
                confidence: 0.95,
                positive_signals: 50,
                negative_signals: 2,
                total_exposures: 52,
            }],
            calibration_insights: vec![],
            decision_outcomes: vec![],
            interaction_patterns: InteractionPatterns {
                total_interactions: 100,
                saves: 20,
                dismissals: 30,
                clicks: 50,
                avg_signal_strength: 0.6,
                top_sources: vec![],
                weekly_velocity: 1.0,
            },
            advantage_trajectory: vec![],
            feedback_stats: FeedbackStats {
                total_items: 500,
                items_with_feedback: 250,
                coverage_pct: 50.0,
                positive_ratio: 0.7,
            },
            detected_tech: vec!["rust".into(), "typescript".into()],
            active_topics: vec![],
        };

        let json = build_developer_context_json(&ctx).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["primary_stack"][0], "rust");
        assert_eq!(parsed["items_processed"], 100);
        assert!(parsed["feedback_coverage_pct"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn test_identity_summary_variants() {
        let base = BehavioralContext {
            topic_affinities: vec![],
            calibration_insights: vec![],
            decision_outcomes: vec![],
            interaction_patterns: InteractionPatterns {
                total_interactions: 0,
                saves: 0,
                dismissals: 0,
                clicks: 0,
                avg_signal_strength: 0.5,
                top_sources: vec![],
                weekly_velocity: 1.0,
            },
            advantage_trajectory: vec![],
            feedback_stats: FeedbackStats {
                total_items: 0,
                items_with_feedback: 0,
                coverage_pct: 0.0,
                positive_ratio: 0.0,
            },
            detected_tech: vec![],
            active_topics: vec![],
        };

        // Empty tech
        assert!(build_identity_summary(&base).contains("Developer"));

        // With tech
        let mut with_tech = base.clone();
        with_tech.detected_tech = vec!["rust".into(), "react".into()];
        assert!(build_identity_summary(&with_tech).contains("rust/react"));

        // High engagement
        let mut active = base.clone();
        active.interaction_patterns.total_interactions = 2000;
        assert!(build_identity_summary(&active).contains("highly active"));
    }
}

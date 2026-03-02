//! STREETS Coach context assembly and shared types.
//!
//! Extracts developer context from Sovereign Profile, Developer DNA,
//! Tech Radar, Decisions, and playbook progress for coaching prompts.

use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachSession {
    pub id: String,
    pub session_type: String,
    pub title: String,
    pub context_snapshot: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachMessage {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub token_count: u64,
    pub cost_cents: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRecommendation {
    pub primary_engine: EngineChoice,
    pub secondary_engine: EngineChoice,
    pub reasoning: String,
    pub profile_gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineChoice {
    pub engine_number: u8,
    pub engine_name: String,
    pub fit_score: f32,
    pub time_to_first_dollar: String,
    pub revenue_range: String,
    pub reasoning: String,
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchReviewResult {
    pub overall_score: f32,
    pub strengths: Vec<String>,
    pub gaps: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachContext {
    pub sovereign_profile_summary: String,
    pub developer_dna_summary: String,
    pub tech_radar_summary: String,
    pub active_decisions: Vec<String>,
    pub playbook_progress: String,
    pub top_engaged_topics: Vec<String>,
    pub profile_completeness: f32,
}

// ============================================================================
// Helpers
// ============================================================================

pub fn generate_session_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut h = DefaultHasher::new();
    nanos.hash(&mut h);
    std::thread::current().id().hash(&mut h);
    format!("coach-{:016x}", h.finish())
}

pub fn require_streets_feature(feature: &str) -> Result<(), String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let license = &guard.get().license;
    if matches!(license.tier.as_str(), "pro" | "team") {
        return Ok(());
    }
    if let Ok(payload) = crate::settings::verify_license_key(&license.license_key) {
        if payload.features.contains(&feature.to_string()) {
            return Ok(());
        }
    }
    Err(format!(
        "{} requires STREETS Community or Cohort membership",
        feature
    ))
}

pub fn ensure_coach_tables(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS coach_sessions (
            id TEXT PRIMARY KEY, session_type TEXT NOT NULL, title TEXT NOT NULL,
            context_snapshot TEXT, created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS coach_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
            role TEXT NOT NULL, content TEXT NOT NULL,
            token_count INTEGER NOT NULL DEFAULT 0, cost_cents INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (session_id) REFERENCES coach_sessions(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS coach_documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT, doc_type TEXT NOT NULL,
            content TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .map_err(|e| format!("Failed to create coach tables: {}", e))
}

pub fn get_llm_client() -> Result<crate::llm::LLMClient, String> {
    let manager = crate::get_settings_manager();
    let guard = manager.lock();
    let provider = guard.get().llm.clone();
    if provider.api_key.is_empty() && provider.provider != "ollama" {
        return Err("LLM not configured -- set up your API key in Settings".to_string());
    }
    Ok(crate::llm::LLMClient::new(provider))
}

/// Record LLM usage and return (total_tokens, cost_cents).
pub fn record_llm_usage(
    client: &crate::llm::LLMClient,
    response: &crate::llm::LLMResponse,
) -> (u64, u64) {
    let total = response.input_tokens + response.output_tokens;
    let cost = client.estimate_cost_cents(response.input_tokens, response.output_tokens);
    let mut settings = crate::get_settings_manager().lock();
    settings.record_usage(total, cost);
    (total, cost)
}

pub fn assemble_coach_context() -> CoachContext {
    let conn = crate::open_db_connection().ok();
    // 1. Sovereign profile
    let sovereign_profile_summary = conn
        .as_ref()
        .and_then(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT category, key, value FROM sovereign_profile ORDER BY category, key",
                )
                .ok()?;
            let rows: Vec<String> = stmt
                .query_map([], |row| {
                    Ok(format!(
                        "{}/{}: {}",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();
            if rows.is_empty() {
                None
            } else {
                Some(rows.join("; "))
            }
        })
        .unwrap_or_default();
    // 2. Developer DNA
    let (developer_dna_summary, top_engaged_topics) = match crate::developer_dna::generate_dna() {
        Ok(dna) => {
            let blind = dna
                .blind_spots
                .iter()
                .take(3)
                .map(|b| b.dependency.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let summary = format!(
                "Identity: {}. Stack: {}. Interests: {}. Blind spots: {}.",
                dna.identity_summary,
                dna.primary_stack.join(", "),
                dna.interests.join(", "),
                blind
            );
            let topics = dna
                .top_engaged_topics
                .iter()
                .take(5)
                .map(|t| t.topic.clone())
                .collect();
            (summary, topics)
        }
        Err(_) => (String::new(), Vec::new()),
    };
    // 3. Tech Radar
    let tech_radar_summary = conn
        .as_ref()
        .and_then(|c| {
            let radar = crate::tech_radar::compute_radar(c).ok()?;
            let by_ring = |ring: &str| {
                radar
                    .entries
                    .iter()
                    .filter(|e| format!("{:?}", e.ring).to_lowercase() == ring)
                    .take(5)
                    .map(|e| e.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            Some(format!(
                "Adopt: [{}]. Trial: [{}]. Assess: [{}]. Hold: [{}].",
                by_ring("adopt"),
                by_ring("trial"),
                by_ring("assess"),
                by_ring("hold")
            ))
        })
        .unwrap_or_default();
    // 4. Active decisions
    let active_decisions = conn
        .as_ref()
        .and_then(|c| {
            let mut stmt = c
                .prepare(
                    "SELECT subject, decision FROM developer_decisions \
                 WHERE status = 'active' LIMIT 10",
                )
                .ok()?;
            let rows: Vec<String> = stmt
                .query_map([], |row| {
                    Ok(format!(
                        "{}: {}",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?
                    ))
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();
            Some(rows)
        })
        .unwrap_or_default();
    // 5. Playbook progress
    let playbook_progress = conn
        .as_ref()
        .map(|c| {
            let n: i64 = c
                .query_row("SELECT COUNT(*) FROM playbook_progress", [], |r| r.get(0))
                .unwrap_or(0);
            format!("{} lessons completed", n)
        })
        .unwrap_or_else(|| "0 lessons completed".to_string());
    // 6. Profile completeness
    let profile_completeness = conn
        .as_ref()
        .map(|c| {
            let filled: i64 = c
                .query_row(
                    "SELECT COUNT(DISTINCT category) FROM sovereign_profile",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            (filled as f32 / 9.0) * 100.0
        })
        .unwrap_or(0.0);

    CoachContext {
        sovereign_profile_summary,
        developer_dna_summary,
        tech_radar_summary,
        active_decisions,
        playbook_progress,
        top_engaged_topics,
        profile_completeness,
    }
}

/// Assemble coach context from the unified Sovereign Developer Profile.
/// Falls back to the direct assembly if profile is unavailable.
#[allow(dead_code)]
pub fn assemble_coach_context_from_profile(
    profile: &crate::sovereign_developer_profile::SovereignDeveloperProfile,
) -> CoachContext {
    let sovereign_profile_summary = {
        let infra = &profile.infrastructure;
        let mut parts = Vec::new();
        if !infra.cpu.is_empty() {
            for (k, v) in &infra.cpu {
                parts.push(format!("cpu/{}: {}", k, v));
            }
        }
        if !infra.gpu.is_empty() {
            for (k, v) in &infra.gpu {
                parts.push(format!("gpu/{}: {}", k, v));
            }
        }
        parts.push(format!("gpu_tier: {}", infra.gpu_tier));
        parts.push(format!("llm_tier: {}", infra.llm_tier));
        parts.join("; ")
    };

    let developer_dna_summary = format!(
        "Identity: {}. Stack: {}. {} dependencies tracked.",
        profile.identity_summary,
        profile.stack.primary_stack.join(", "),
        profile.stack.dependencies.len()
    );

    let tech_radar_summary = {
        let r = &profile.preferences.tech_radar;
        format!(
            "Adopt: [{}]. Trial: [{}]. Assess: [{}]. Hold: [{}].",
            r.adopt.join(", "),
            r.trial.join(", "),
            r.assess.join(", "),
            r.hold.join(", ")
        )
    };

    let active_decisions = profile
        .preferences
        .active_decisions
        .iter()
        .map(|d| format!("{}: {}", d.subject, d.decision))
        .collect();

    let pp = &profile.skills.playbook_progress;
    let playbook_progress = format!(
        "{}/{} lessons completed",
        pp.completed_lessons, pp.total_lessons
    );

    let top_engaged_topics = profile
        .skills
        .top_affinities
        .iter()
        .take(5)
        .map(|a| a.topic.clone())
        .collect();

    let profile_completeness = profile.completeness.overall_percentage as f32;

    CoachContext {
        sovereign_profile_summary,
        developer_dna_summary,
        tech_radar_summary,
        active_decisions,
        playbook_progress,
        top_engaged_topics,
        profile_completeness,
    }
}

pub fn format_system_prompt(session_type: &str, ctx: &CoachContext) -> String {
    let or_default = |s: &str, d: &str| {
        if s.is_empty() {
            d.to_string()
        } else {
            s.to_string()
        }
    };
    let decisions_str = if ctx.active_decisions.is_empty() {
        "None recorded".into()
    } else {
        ctx.active_decisions.join("; ")
    };
    let topics_str = if ctx.top_engaged_topics.is_empty() {
        "None yet".into()
    } else {
        ctx.top_engaged_topics.join(", ")
    };
    let context_block = format!(
        "## Developer Profile\nSovereign Profile: {}\nDeveloper DNA: {}\nTech Radar: {}\n\
         Active Decisions: {}\nPlaybook Progress: {}\nTop Topics: {}\nProfile Completeness: {:.0}%",
        or_default(&ctx.sovereign_profile_summary, "Not yet configured"),
        or_default(&ctx.developer_dna_summary, "Not enough data yet"),
        or_default(&ctx.tech_radar_summary, "No radar data yet"),
        decisions_str,
        ctx.playbook_progress,
        topics_str,
        ctx.profile_completeness
    );

    let role_text = match session_type {
        "engine_recommender" => {
            "You are the STREETS Revenue Engine Recommender. Analyze the developer's profile and recommend \
            the best-fit revenue engines from: 1) Open Source + Sponsorship 2) Technical Writing 3) Freelance/Consulting \
            4) SaaS/Micro-SaaS 5) Developer Tools 6) Education/Courses 7) API/Infrastructure 8) Productized Services.\n\n\
            You MUST respond with valid JSON: {\"primary_engine\":{\"engine_number\":N,\"engine_name\":\"...\",\"fit_score\":0.0-1.0,\
            \"time_to_first_dollar\":\"...\",\"revenue_range\":\"...\",\"reasoning\":\"...\",\"prerequisites\":[...]},\
            \"secondary_engine\":{...},\"reasoning\":\"...\",\"profile_gaps\":[...]}"
        }
        "strategy" => {
            "You are the STREETS Strategic Advisor. Generate a comprehensive personalized strategy in markdown. \
            Cover: current position analysis, recommended revenue engines, 90-day roadmap, key risks, specific next actions. \
            Base everything on the developer's actual data. Be specific, not generic."
        }
        "launch_review" => {
            "You are the STREETS Launch Reviewer. Evaluate project readiness. MUST respond with valid JSON: \
            {\"overall_score\":0-100,\"strengths\":[...],\"gaps\":[...],\"recommendations\":[...]}. \
            Score should be 0-100 (integer). \
            Score on: technical readiness, market positioning, revenue model clarity, operational maturity, differentiation."
        }
        "progress" => {
            "You are the STREETS Progress Coach. Analyze playbook progress and autonomy journey. Provide: \
            progress summary, bottlenecks, specific next steps, motivational context. Respond in markdown. Be encouraging but honest."
        }
        _ => {
            "You are the STREETS Coach -- a personalized AI advisor for developer autonomy and revenue generation. \
            You know the STREETS framework (Sovereign Setup, Technical Moats, Revenue Engines, Execution, Evolving Edge, \
            Tactical Automation, Stacking Streams). Give specific, actionable advice referencing their actual stack, decisions, and progress."
        }
    };
    format!("{}\n\n{}", role_text, context_block)
}

pub fn extract_json_from_response(response: &str) -> &str {
    if let Some(start) = response.find("```json") {
        let after = &response[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    if let Some(start) = response.find("```") {
        let after = &response[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('{') || inner.starts_with('[') {
                return inner;
            }
        }
    }
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            return &response[start..=end];
        }
    }
    response
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id_format() {
        let id = generate_session_id();
        assert!(id.starts_with("coach-"));
        assert_eq!(id.len(), 22); // "coach-" (6) + 16 hex chars
    }

    #[test]
    fn test_extract_json_from_fenced_response() {
        let response = "Here is the analysis:\n```json\n{\"score\": 7.5}\n```\nDone.";
        assert_eq!(extract_json_from_response(response), "{\"score\": 7.5}");
    }

    #[test]
    fn test_extract_json_raw_object() {
        let response = "The result is {\"key\": \"value\"} as shown.";
        assert_eq!(extract_json_from_response(response), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_extract_json_no_json() {
        let response = "No JSON here at all.";
        assert_eq!(extract_json_from_response(response), response);
    }

    #[test]
    fn test_format_system_prompt_chat() {
        let ctx = CoachContext {
            sovereign_profile_summary: "cpu/model: AMD Ryzen".to_string(),
            developer_dna_summary: "Rust/TS developer".to_string(),
            tech_radar_summary: "Adopt: [rust, typescript]".to_string(),
            active_decisions: vec!["sqlite: Use SQLite".to_string()],
            playbook_progress: "3 lessons completed".to_string(),
            top_engaged_topics: vec!["rust".to_string(), "wasm".to_string()],
            profile_completeness: 44.0,
        };
        let prompt = format_system_prompt("chat", &ctx);
        assert!(prompt.contains("STREETS Coach"));
        assert!(prompt.contains("AMD Ryzen"));
        assert!(prompt.contains("Rust/TS developer"));
        assert!(prompt.contains("44%"));
    }

    #[test]
    fn test_format_system_prompt_engine_recommender() {
        let ctx = CoachContext {
            sovereign_profile_summary: String::new(),
            developer_dna_summary: String::new(),
            tech_radar_summary: String::new(),
            active_decisions: Vec::new(),
            playbook_progress: "0 lessons completed".to_string(),
            top_engaged_topics: Vec::new(),
            profile_completeness: 0.0,
        };
        let prompt = format_system_prompt("engine_recommender", &ctx);
        assert!(prompt.contains("Revenue Engine Recommender"));
        assert!(prompt.contains("engine_number"));
        assert!(prompt.contains("Not yet configured"));
    }
}

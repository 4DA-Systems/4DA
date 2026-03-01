//! Calibration Commands — 4-dimension rig calibration and scoring quality assessment.
//!
//! Grade = Infrastructure (25) + Context Richness (25) + Signal Coverage (25) + Discrimination (25)
//! Each recommendation carries an `action_type` for one-click frontend actions.

use serde::Serialize;
use ts_rs::TS;

use crate::calibration_probes;

// ============================================================================
// Public Types (exported to frontend via ts-rs)
// ============================================================================

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CalibrationResult {
    pub grade: String,
    pub grade_score: u32,
    pub aggregate_f1: f64,
    pub aggregate_precision: f64,
    pub aggregate_recall: f64,
    pub mean_separation_gap: f64,
    pub corpus_items: u32,
    pub personas_tested: u32,
    pub per_persona: Vec<PersonaMetrics>,
    pub worst_persona: String,
    pub best_persona: String,
    pub rig_requirements: RigRequirements,
    pub recommendations: Vec<Recommendation>,
    // 4-dimension scores (new)
    pub infrastructure_score: u32,
    pub context_richness_score: u32,
    pub signal_coverage_score: u32,
    pub discrimination_score: u32,
    pub active_signal_axes: Vec<String>,
    pub nearest_persona: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct PersonaMetrics {
    pub name: String,
    pub display_name: String,
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub separation_gap: f64,
    pub tp: u32,
    pub fp: u32,
    pub tn: u32,
    pub r#fn: u32,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RigRequirements {
    pub ollama_running: bool,
    pub ollama_url: String,
    pub embedding_model: Option<String>,
    pub embedding_available: bool,
    pub gpu_detected: bool,
    pub recommended_model: String,
    pub estimated_ram_gb: f64,
    pub can_reach_grade_a: bool,
    pub grade_a_requirements: Vec<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct Recommendation {
    pub priority: String,
    pub title: String,
    pub description: String,
    pub action: Option<String>,
    pub action_type: Option<String>,
}

// ============================================================================
// Legacy Metrics (kept for simulation utilities and test coverage)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
    Strong,
    Weak,
    Borderline,
    Noise,
}

#[allow(dead_code)]
struct Metrics {
    tp: u32,
    fp: u32,
    tn: u32,
    r#fn: u32,
    relevant_scores: Vec<f64>,
    noise_scores: Vec<f64>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            tp: 0,
            fp: 0,
            tn: 0,
            r#fn: 0,
            relevant_scores: Vec::new(),
            noise_scores: Vec::new(),
        }
    }
    fn record(&mut self, score: f32, relevant: bool, expected: Expected) {
        let s = score as f64;
        match expected {
            Expected::Strong | Expected::Weak => {
                self.relevant_scores.push(s);
                if relevant {
                    self.tp += 1;
                } else {
                    self.r#fn += 1;
                }
            }
            Expected::Noise => {
                self.noise_scores.push(s);
                if relevant {
                    self.fp += 1;
                } else {
                    self.tn += 1;
                }
            }
            Expected::Borderline => {}
        }
    }
    fn precision(&self) -> f64 {
        let d = (self.tp + self.fp) as f64;
        if d == 0.0 {
            1.0
        } else {
            self.tp as f64 / d
        }
    }
    fn recall(&self) -> f64 {
        let d = (self.tp + self.r#fn) as f64;
        if d == 0.0 {
            1.0
        } else {
            self.tp as f64 / d
        }
    }
    fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
    fn separation_gap(&self) -> f64 {
        let mean_rel = if self.relevant_scores.is_empty() {
            0.0
        } else {
            self.relevant_scores.iter().sum::<f64>() / self.relevant_scores.len() as f64
        };
        let mean_noise = if self.noise_scores.is_empty() {
            0.0
        } else {
            self.noise_scores.iter().sum::<f64>() / self.noise_scores.len() as f64
        };
        mean_rel - mean_noise
    }
    fn merge(&mut self, other: &Metrics) {
        self.tp += other.tp;
        self.fp += other.fp;
        self.tn += other.tn;
        self.r#fn += other.r#fn;
        self.relevant_scores
            .extend_from_slice(&other.relevant_scores);
        self.noise_scores.extend_from_slice(&other.noise_scores);
    }
}

// ============================================================================
// Persona Display
// ============================================================================

#[allow(dead_code)]
fn persona_display_name(name: &str) -> String {
    match name {
        "rust_systems" => "Rust / Systems",
        "python_ml" => "Python / ML",
        "fullstack_ts" => "Full-Stack TypeScript",
        "devops_sre" => "DevOps / SRE",
        "mobile_dev" => "Mobile Developer",
        "bootstrap" => "First-Run (Bootstrap)",
        "power_user" => "Power User",
        "context_switcher" => "Context Switcher",
        "niche_specialist" => "Niche Specialist",
        _ => name,
    }
    .to_string()
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn run_calibration() -> Result<CalibrationResult, String> {
    let db = crate::get_database()?;

    // Build the user's actual scoring context
    let ctx = crate::scoring::build_scoring_context(&db)
        .await
        .map_err(|e| format!("Context build failed: {e}"))?;

    // Check Ollama/embedding status
    let rig = check_rig_requirements().await;

    // === 4-Dimension Scoring ===

    // Dimension 1: Infrastructure
    let infra_score = calibration_probes::compute_infrastructure_score(&rig);

    // Dimension 2: Context Richness
    let context_score = calibration_probes::compute_context_score(&ctx);

    // Dimension 3: Signal Coverage (audit which axes fire)
    let audit = calibration_probes::audit_signal_axes(&ctx, &db);
    let signal_score = calibration_probes::compute_signal_score(&audit);

    // Dimension 4: Discrimination (run domain-aware probes)
    let probe_results = calibration_probes::run_probe_calibration(&ctx, &db);
    let disc_score = calibration_probes::compute_discrimination_score(&probe_results);

    // Compute grade from 4 dimensions
    let (grade, grade_score) = calibration_probes::compute_grade_from_dimensions(
        infra_score,
        context_score,
        signal_score,
        disc_score,
    );

    // Detect nearest persona
    let domain = calibration_probes::detect_user_domain(&ctx);
    let nearest_persona = calibration_probes::domain_name(domain).to_string();

    // === Build Recommendations (with action_type) ===
    let mut recommendations = Vec::new();

    if !rig.ollama_running {
        recommendations.push(Recommendation {
            priority: "P0".into(),
            title: "Install and run Ollama".into(),
            description: "Ollama provides local embeddings that dramatically improve scoring accuracy. Without it, scoring relies on keyword matching only.".into(),
            action: Some("Install from https://ollama.com and run: ollama pull nomic-embed-text".into()),
            action_type: Some("install_ollama".into()),
        });
    } else if !rig.embedding_available {
        recommendations.push(Recommendation {
            priority: "P0".into(),
            title: "Pull an embedding model".into(),
            description: "Ollama is running but no embedding model is available. Pull one to enable semantic scoring.".into(),
            action: Some("ollama pull nomic-embed-text".into()),
            action_type: Some("pull_embedding_model".into()),
        });
    }

    if ctx.interest_count == 0 {
        recommendations.push(Recommendation {
            priority: "P0".into(),
            title: "Add your interests".into(),
            description: "The scoring engine needs to know what you care about. Add at least 3 interests in Settings.".into(),
            action: Some("Open Settings → Interests".into()),
            action_type: Some("open_settings_interests".into()),
        });
    } else if ctx.interest_count < 3 {
        recommendations.push(Recommendation {
            priority: "P1".into(),
            title: "Add more interests".into(),
            description: format!(
                "You have {} interest(s). Adding 3+ gives the engine enough signal to separate relevant content from noise.",
                ctx.interest_count
            ),
            action: Some("Open Settings → Interests".into()),
            action_type: Some("open_settings_interests".into()),
        });
    }

    if !ctx.composed_stack.active {
        recommendations.push(Recommendation {
            priority: "P1".into(),
            title: "Auto-detect your stack".into(),
            description: "Stack profiles tell the engine your tech identity. Auto-detect from your projects or select manually.".into(),
            action: Some("Open Settings → Stack Profiles".into()),
            action_type: Some("auto_detect_stacks".into()),
        });
    }

    if ctx.feedback_interaction_count < 10 {
        recommendations.push(Recommendation {
            priority: "P1".into(),
            title: "Use feedback buttons".into(),
            description: format!(
                "You've given {} feedback interactions. The engine learns from your thumbs up/down — 10+ interactions significantly improve accuracy.",
                ctx.feedback_interaction_count
            ),
            action: None,
            action_type: Some("give_feedback".into()),
        });
    }

    for failure in &probe_results.failures {
        recommendations.push(Recommendation {
            priority: "P2".into(),
            title: "Probe classification miss".into(),
            description: failure.clone(),
            action: None,
            action_type: None,
        });
    }

    // Build per-persona metrics from probe data
    let per_persona = vec![PersonaMetrics {
        name: "your_profile".into(),
        display_name: "Your Profile".into(),
        f1: probe_results.f1,
        precision: probe_results.precision,
        recall: probe_results.recall,
        separation_gap: probe_results.separation_gap,
        tp: probe_results.passed,
        fp: 0,
        tn: probe_results.total.saturating_sub(probe_results.passed),
        r#fn: 0,
    }];

    Ok(CalibrationResult {
        grade,
        grade_score,
        aggregate_f1: probe_results.f1,
        aggregate_precision: probe_results.precision,
        aggregate_recall: probe_results.recall,
        mean_separation_gap: probe_results.separation_gap,
        corpus_items: probe_results.total,
        personas_tested: 1,
        per_persona,
        worst_persona: "your_profile".into(),
        best_persona: "your_profile".into(),
        rig_requirements: rig,
        recommendations,
        infrastructure_score: infra_score,
        context_richness_score: context_score,
        signal_coverage_score: signal_score,
        discrimination_score: disc_score,
        active_signal_axes: audit.axes,
        nearest_persona,
    })
}

pub(crate) async fn check_rig_requirements() -> RigRequirements {
    let ollama_url = "http://localhost:11434".to_string();

    let ollama_check = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .and_then(|c| Ok(c.get(format!("{}/api/tags", ollama_url))));

    let (ollama_running, embedding_model, models_list) = match ollama_check {
        Ok(req) => match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let models: Vec<String> = body["models"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|m| m["name"].as_str().map(String::from))
                    .collect();
                let embed_model = models
                    .iter()
                    .find(|m| {
                        m.contains("nomic-embed")
                            || m.contains("mxbai-embed")
                            || m.contains("all-minilm")
                    })
                    .cloned();
                (true, embed_model, models)
            }
            _ => (false, None, vec![]),
        },
        Err(_) => (false, None, vec![]),
    };

    let embedding_available = embedding_model.is_some();
    let gpu_detected = models_list.len() > 2;

    let mut grade_a_requirements = Vec::new();
    if !ollama_running {
        grade_a_requirements.push("Install and run Ollama (https://ollama.com)".into());
    }
    if !embedding_available {
        grade_a_requirements.push("Pull embedding model: ollama pull nomic-embed-text".into());
    }
    grade_a_requirements.push("Add 3+ interests in Settings".into());
    grade_a_requirements.push("Give 10+ feedback interactions (thumbs up/down)".into());
    grade_a_requirements.push("Select 1-3 stack profiles".into());

    RigRequirements {
        ollama_running,
        ollama_url,
        embedding_model,
        embedding_available,
        gpu_detected,
        recommended_model: "nomic-embed-text".into(),
        estimated_ram_gb: if gpu_detected { 8.0 } else { 4.0 },
        can_reach_grade_a: ollama_running && embedding_available,
        grade_a_requirements,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "calibration_tests.rs"]
mod tests;

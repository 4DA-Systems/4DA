//! No-LLM Computation Engine — structured data cards from sovereign data.
//!
//! Produces `SovereignInsightCard` structs with data points and visualizations
//! computed entirely from local sovereign data. No LLM calls required.

use super::context::PersonalizationContext;
use super::{
    BarEntry, BlockPosition, CardType, ConnectionType, DataPoint, InsightBlock, InsightContent,
    MirrorBlock, RankItem, SovereignInsightCard, Visualization,
};

use tracing::debug;

// ============================================================================
// Static Data: Engine Requirements
// ============================================================================

/// Revenue engine definitions with hardware/stack requirements.
struct EngineSpec {
    id: u8,
    name: &'static str,
    min_ram_gb: f64,
    needs_gpu: bool,
    prereq_stacks: &'static [&'static str],
    topics: &'static [&'static str],
}

const ENGINES: &[EngineSpec] = &[
    EngineSpec {
        id: 1,
        name: "Open Source + Sponsorship",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &["git"],
        topics: &["open-source", "github", "community"],
    },
    EngineSpec {
        id: 2,
        name: "Technical Writing",
        min_ram_gb: 4.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["writing", "blog", "documentation", "tutorial"],
    },
    EngineSpec {
        id: 3,
        name: "Freelance / Consulting",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["freelance", "consulting", "client", "contract"],
    },
    EngineSpec {
        id: 4,
        name: "SaaS / Micro-SaaS",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &["react", "typescript", "javascript", "python", "node"],
        topics: &["saas", "startup", "product", "subscription"],
    },
    EngineSpec {
        id: 5,
        name: "Developer Tools",
        min_ram_gb: 16.0,
        needs_gpu: false,
        prereq_stacks: &["rust", "go", "typescript", "python"],
        topics: &["devtools", "cli", "sdk", "api", "tooling"],
    },
    EngineSpec {
        id: 6,
        name: "Education / Courses",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["education", "course", "teaching", "workshop"],
    },
    EngineSpec {
        id: 7,
        name: "API / Infrastructure",
        min_ram_gb: 16.0,
        needs_gpu: true,
        prereq_stacks: &["python", "rust", "go", "typescript"],
        topics: &["api", "infrastructure", "cloud", "backend"],
    },
    EngineSpec {
        id: 8,
        name: "Productized Services",
        min_ram_gb: 8.0,
        needs_gpu: false,
        prereq_stacks: &[],
        topics: &["service", "automation", "productized"],
    },
];

// ============================================================================
// L3: Insight Block Computation
// ============================================================================

/// Compute L3 insight blocks for the given injection markers.
pub fn compute_insight_blocks(
    markers: &[String],
    ctx: &PersonalizationContext,
    _module_id: &str,
    _lesson_idx: u32,
) -> Vec<InsightBlock> {
    markers
        .iter()
        .filter_map(|marker| compute_single_insight(marker, ctx))
        .collect()
}

fn compute_single_insight(marker_id: &str, ctx: &PersonalizationContext) -> Option<InsightBlock> {
    let card = match marker_id {
        "hardware_benchmark" => compute_hardware_benchmark(ctx),
        "stack_fit" => compute_stack_fit(ctx),
        "cost_projection" => compute_cost_projection(ctx),
        "t_shape" => compute_t_shape(ctx),
        "engine_ranking" => compute_engine_ranking(ctx),
        "blind_spot_alert" => compute_blind_spot_alert(ctx),
        _ => {
            debug!(target: "4da::personalize", marker = marker_id, "Unknown insight marker");
            return None;
        }
    };

    let card = card?;
    let source_labels = infer_sources(marker_id, ctx);
    let confidence = compute_confidence(marker_id, ctx);

    Some(InsightBlock {
        block_id: marker_id.to_string(),
        position: BlockPosition::Injection {
            marker_id: marker_id.to_string(),
        },
        content: InsightContent::Card(card),
        source_labels,
        confidence,
    })
}

// ============================================================================
// Card Computation Functions
// ============================================================================

/// Hardware benchmark card: GPU/RAM/storage vs engine requirements.
fn compute_hardware_benchmark(ctx: &PersonalizationContext) -> Option<SovereignInsightCard> {
    let ram_str = ctx.profile.ram.get("total").cloned().unwrap_or_default();
    let ram_gb = parse_gb(&ram_str);
    let gpu_name = ctx
        .profile
        .gpu
        .get("name")
        .cloned()
        .unwrap_or_else(|| "Not detected".into());
    let gpu_mem_str = ctx
        .profile
        .gpu
        .get("memory_total")
        .cloned()
        .unwrap_or_default();
    let gpu_mem_gb = parse_gb(&gpu_mem_str);
    let cpu_model = ctx
        .profile
        .cpu
        .get("model")
        .cloned()
        .unwrap_or_else(|| "Unknown".into());
    let cpu_cores = ctx.profile.cpu.get("cores").cloned().unwrap_or_default();

    if ram_gb == 0.0 && gpu_mem_gb == 0.0 && cpu_model == "Unknown" {
        return None; // No hardware data at all
    }

    let mut data_points = vec![
        DataPoint {
            label: "CPU".into(),
            value: if cpu_cores.is_empty() {
                cpu_model.clone()
            } else {
                format!("{} ({} cores)", cpu_model, cpu_cores)
            },
            context: None,
            highlight: false,
        },
        DataPoint {
            label: "RAM".into(),
            value: if ram_gb > 0.0 {
                format!("{:.0} GB", ram_gb)
            } else {
                "Not detected".into()
            },
            context: Some(ram_tier_context(ram_gb)),
            highlight: ram_gb >= 32.0,
        },
        DataPoint {
            label: "GPU".into(),
            value: gpu_name.clone(),
            context: if gpu_mem_gb > 0.0 {
                Some(format!("{:.0} GB VRAM", gpu_mem_gb))
            } else {
                None
            },
            highlight: ctx.computed.has_nvidia,
        },
    ];

    // Add LLM inference capability assessment
    let llm_note = if gpu_mem_gb >= 24.0 {
        "Can run 70B+ parameter models locally"
    } else if gpu_mem_gb >= 12.0 {
        "Can run 13B-34B parameter models locally"
    } else if gpu_mem_gb >= 8.0 {
        "Can run 7B-13B parameter models locally"
    } else if gpu_mem_gb >= 4.0 {
        "Can run small (3B-7B) models locally"
    } else {
        "Cloud LLM recommended for best experience"
    };

    data_points.push(DataPoint {
        label: "Local LLM Capacity".into(),
        value: llm_note.into(),
        context: None,
        highlight: gpu_mem_gb >= 12.0,
    });

    // Bar chart: RAM and VRAM as bars
    let visualization = if ram_gb > 0.0 {
        Some(Visualization::BarChart {
            bars: vec![
                BarEntry {
                    label: "RAM".into(),
                    value: ram_gb,
                    highlight: ram_gb >= 32.0,
                },
                BarEntry {
                    label: "VRAM".into(),
                    value: gpu_mem_gb,
                    highlight: gpu_mem_gb >= 12.0,
                },
            ],
            max_value: 64.0,
            unit: "GB".into(),
        })
    } else {
        None
    };

    Some(SovereignInsightCard {
        card_type: CardType::HardwareBenchmark,
        title: "Your Hardware Profile".into(),
        data_points,
        visualization,
    })
}

/// Stack fit card: how well the user's stack matches each engine.
fn compute_stack_fit(ctx: &PersonalizationContext) -> Option<SovereignInsightCard> {
    if ctx.stack.primary.is_empty() {
        return None;
    }

    let scores: Vec<(u8, &str, f64)> = ENGINES
        .iter()
        .map(|e| {
            let score = engine_stack_score(e, ctx);
            (e.id, e.name, score)
        })
        .collect();

    let mut sorted = scores.clone();
    sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let data_points = sorted
        .iter()
        .take(3)
        .map(|(id, name, score)| DataPoint {
            label: format!("Engine {}", id),
            value: name.to_string(),
            context: Some(format!("{:.0}% stack match", score * 100.0)),
            highlight: *score >= 0.7,
        })
        .collect();

    let visualization = Some(Visualization::RankList {
        items: sorted
            .iter()
            .map(|(_, name, score)| RankItem {
                rank: 0, // Assigned below
                name: name.to_string(),
                score: *score,
                matches_stack: *score >= 0.5,
            })
            .enumerate()
            .map(|(i, mut item)| {
                item.rank = (i + 1) as u32;
                item
            })
            .collect(),
    });

    Some(SovereignInsightCard {
        card_type: CardType::StackFit,
        title: "Engine × Stack Fit".into(),
        data_points,
        visualization,
    })
}

/// Cost projection card: monthly operating costs in user's currency.
fn compute_cost_projection(ctx: &PersonalizationContext) -> Option<SovereignInsightCard> {
    if ctx.regional.electricity_kwh == 0.0 && ctx.regional.internet_monthly == 0.0 {
        return None;
    }

    let electricity_monthly =
        ctx.computed.monthly_electricity_estimate * ctx.regional.electricity_kwh;
    let internet = ctx.regional.internet_monthly;
    let total = electricity_monthly + internet;

    let symbol = &ctx.regional.currency_symbol;

    let data_points = vec![
        DataPoint {
            label: "Electricity (est.)".into(),
            value: format!("{}{:.2}/mo", symbol, electricity_monthly),
            context: Some(format!(
                "{:.1} kWh × {}{:.3}/kWh",
                ctx.computed.monthly_electricity_estimate, symbol, ctx.regional.electricity_kwh
            )),
            highlight: false,
        },
        DataPoint {
            label: "Internet".into(),
            value: format!("{}{:.0}/mo", symbol, internet),
            context: None,
            highlight: false,
        },
        DataPoint {
            label: "Base Operating Cost".into(),
            value: format!("{}{:.2}/mo", symbol, total),
            context: Some("Before API costs or hosting".into()),
            highlight: true,
        },
    ];

    Some(SovereignInsightCard {
        card_type: CardType::CostProjection,
        title: format!("Monthly Operating Costs ({})", ctx.regional.currency),
        data_points,
        visualization: None,
    })
}

/// T-shape diagram card: primary depth + breadth of adjacent tech.
fn compute_t_shape(ctx: &PersonalizationContext) -> Option<SovereignInsightCard> {
    if ctx.stack.primary.is_empty() {
        return None;
    }

    let primary = ctx.stack.primary.first().cloned().unwrap_or_default();
    let adjacent = ctx.stack.adjacent.clone();

    let data_points = vec![
        DataPoint {
            label: "Primary Depth".into(),
            value: primary.clone(),
            context: Some("Your deepest expertise".into()),
            highlight: true,
        },
        DataPoint {
            label: "Breadth".into(),
            value: format!("{} adjacent technologies", adjacent.len()),
            context: Some(
                if adjacent.len() >= 5 {
                    "Strong cross-domain reach"
                } else {
                    "Room to expand breadth"
                }
                .into(),
            ),
            highlight: adjacent.len() >= 5,
        },
    ];

    Some(SovereignInsightCard {
        card_type: CardType::TShapeDiagram,
        title: "Your T-Shape".into(),
        data_points,
        visualization: Some(Visualization::TShape {
            primary: primary.clone(),
            depth_label: format!("{} (primary)", primary),
            adjacent: adjacent.iter().take(8).cloned().collect(),
            breadth_label: format!("{} technologies", adjacent.len()),
        }),
    })
}

/// Engine ranking card: all 8 engines ranked by overall fit score.
fn compute_engine_ranking(ctx: &PersonalizationContext) -> Option<SovereignInsightCard> {
    if ctx.stack.primary.is_empty() && ctx.profile.categories_filled == 0 {
        return None;
    }

    let mut ranked: Vec<(u8, &str, f64)> = ENGINES
        .iter()
        .map(|e| {
            let stack_score = engine_stack_score(e, ctx);
            let hw_score = engine_hardware_score(e, ctx);
            let combined = stack_score * 0.6 + hw_score * 0.4;
            (e.id, e.name, combined)
        })
        .collect();

    ranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let data_points = ranked
        .iter()
        .take(3)
        .map(|(id, name, score)| DataPoint {
            label: format!(
                "#{} — Engine {}",
                ranked.iter().position(|r| r.0 == *id).unwrap_or(0) + 1,
                id
            ),
            value: name.to_string(),
            context: Some(format!("{:.0}% fit", score * 100.0)),
            highlight: *score >= 0.7,
        })
        .collect();

    let visualization = Some(Visualization::RankList {
        items: ranked
            .iter()
            .enumerate()
            .map(|(i, (_, name, score))| RankItem {
                rank: (i + 1) as u32,
                name: name.to_string(),
                score: *score,
                matches_stack: *score >= 0.5,
            })
            .collect(),
    });

    Some(SovereignInsightCard {
        card_type: CardType::EngineRanking,
        title: "Revenue Engines — Your Fit Ranking".into(),
        data_points,
        visualization,
    })
}

/// Blind spot alert card: dependencies you use but don't track.
fn compute_blind_spot_alert(ctx: &PersonalizationContext) -> Option<SovereignInsightCard> {
    if ctx.dna.blind_spots.is_empty() {
        return None;
    }

    let data_points = ctx
        .dna
        .blind_spots
        .iter()
        .take(5)
        .map(|bs| DataPoint {
            label: "Untracked Dependency".into(),
            value: bs.clone(),
            context: Some("You use this but don't follow its ecosystem".into()),
            highlight: true,
        })
        .collect();

    Some(SovereignInsightCard {
        card_type: CardType::BlindSpotAlert,
        title: "Knowledge Blind Spots".into(),
        data_points,
        visualization: None,
    })
}

// ============================================================================
// L4: Mirror / Sovereign Connection Blocks
// ============================================================================

/// Compute L4 cross-data connection blocks.
pub fn compute_mirror_blocks(
    ctx: &PersonalizationContext,
    _module_id: &str,
    _lesson_idx: u32,
) -> Vec<MirrorBlock> {
    let mut blocks = Vec::new();

    // Connection 1: Blind spot → moat opportunity
    if let Some(block) = compute_blind_spot_moat(ctx) {
        blocks.push(block);
    }

    // Connection 2: Feed engagement → engine prediction
    if let Some(block) = compute_feed_predicts_engine(ctx) {
        blocks.push(block);
    }

    // Connection 3: Radar momentum → timing signal
    if let Some(block) = compute_radar_momentum(ctx) {
        blocks.push(block);
    }

    blocks
}

fn compute_blind_spot_moat(ctx: &PersonalizationContext) -> Option<MirrorBlock> {
    if ctx.dna.blind_spots.is_empty() || ctx.stack.primary.is_empty() {
        return None;
    }

    let blind_spot = ctx.dna.blind_spots.first()?;
    let primary = ctx.stack.primary.first()?;

    Some(MirrorBlock {
        block_id: "blind_spot_moat".into(),
        connection_type: ConnectionType::BlindSpotMoat,
        headline: format!("{} × {} = Unexplored Moat", blind_spot, primary),
        insight: format!(
            "You deeply know {} but don't track {}. Developers who bridge this gap \
             often find unique product opportunities that pure specialists miss.",
            primary, blind_spot
        ),
        data_sources: vec!["Developer DNA".into(), "Domain Profile".into()],
        content: None,
    })
}

fn compute_feed_predicts_engine(ctx: &PersonalizationContext) -> Option<MirrorBlock> {
    if ctx.dna.top_engaged_topics.is_empty() {
        return None;
    }

    // Find which engine's topics match the user's top engaged topics
    let mut best_match: Option<(&str, usize)> = None;
    for engine in ENGINES {
        let matches = engine
            .topics
            .iter()
            .filter(|t| {
                ctx.dna
                    .top_engaged_topics
                    .iter()
                    .any(|et| et.to_lowercase().contains(*t))
            })
            .count();
        if matches > 0 {
            if best_match.map(|(_, c)| matches > c).unwrap_or(true) {
                best_match = Some((engine.name, matches));
            }
        }
    }

    let (engine_name, match_count) = best_match?;

    Some(MirrorBlock {
        block_id: "feed_predicts_engine".into(),
        connection_type: ConnectionType::FeedPredictsEngine,
        headline: format!("Your Reading Habits Point to: {}", engine_name),
        insight: format!(
            "Your most-engaged feed topics match {} topic signals for {}. \
             Your attention naturally gravitates toward this engine's domain.",
            match_count, engine_name
        ),
        data_sources: vec!["Feed Engagement".into(), "Engine Topic Mapping".into()],
        content: None,
    })
}

fn compute_radar_momentum(ctx: &PersonalizationContext) -> Option<MirrorBlock> {
    if ctx.radar.trial.is_empty() && ctx.radar.assess.is_empty() {
        return None;
    }

    let rising_tech: Vec<&String> = ctx
        .radar
        .trial
        .iter()
        .chain(ctx.radar.assess.iter())
        .take(3)
        .collect();

    if rising_tech.is_empty() {
        return None;
    }

    let tech_list = rising_tech
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    Some(MirrorBlock {
        block_id: "radar_momentum".into(),
        connection_type: ConnectionType::RadarMomentum,
        headline: format!("Rising in Your Radar: {}", tech_list),
        insight: format!(
            "Technologies in your Trial/Assess rings ({}) suggest emerging interest. \
             Products built on emerging tech you're actively learning have stronger moats.",
            tech_list
        ),
        data_sources: vec!["Tech Radar".into()],
        content: None,
    })
}

// ============================================================================
// Scoring Helpers
// ============================================================================

/// Score how well the user's stack matches an engine's prerequisites.
fn engine_stack_score(engine: &EngineSpec, ctx: &PersonalizationContext) -> f64 {
    if engine.prereq_stacks.is_empty() {
        return 0.5; // No specific stack required
    }

    let all_tech: Vec<String> = ctx
        .stack
        .primary
        .iter()
        .chain(ctx.stack.adjacent.iter())
        .map(|s| s.to_lowercase())
        .collect();

    let matched = engine
        .prereq_stacks
        .iter()
        .filter(|req| all_tech.iter().any(|t| t.contains(*req)))
        .count();

    (matched as f64 / engine.prereq_stacks.len() as f64).min(1.0)
}

/// Score how well the user's hardware matches an engine's requirements.
fn engine_hardware_score(engine: &EngineSpec, ctx: &PersonalizationContext) -> f64 {
    let ram_str = ctx.profile.ram.get("total").cloned().unwrap_or_default();
    let ram_gb = parse_gb(&ram_str);

    let mut score = 1.0;

    // RAM check
    if ram_gb > 0.0 && ram_gb < engine.min_ram_gb {
        score *= ram_gb / engine.min_ram_gb;
    }

    // GPU check
    if engine.needs_gpu && ctx.computed.gpu_tier == "none" {
        score *= 0.5;
    }

    score.min(1.0)
}

/// Parse a string like "32 GB" or "16384 MB" into GB as f64.
fn parse_gb(s: &str) -> f64 {
    let s = s.trim().to_lowercase();
    if let Some(gb) = s.strip_suffix("gb") {
        gb.trim().parse().unwrap_or(0.0)
    } else if let Some(mb) = s.strip_suffix("mb") {
        mb.trim().parse::<f64>().unwrap_or(0.0) / 1024.0
    } else if let Some(tb) = s.strip_suffix("tb") {
        tb.trim().parse::<f64>().unwrap_or(0.0) * 1024.0
    } else {
        // Try parsing as plain number (assume GB)
        s.split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(0.0)
    }
}

/// RAM tier description for context.
fn ram_tier_context(gb: f64) -> String {
    if gb >= 64.0 {
        "Workstation-grade — can run multiple large models simultaneously".into()
    } else if gb >= 32.0 {
        "Strong — comfortable for most local AI workloads".into()
    } else if gb >= 16.0 {
        "Adequate — can run small-medium local models".into()
    } else if gb >= 8.0 {
        "Minimum for development — cloud LLM recommended".into()
    } else {
        "Limited — cloud LLM strongly recommended".into()
    }
}

/// Infer which data sources contributed to a given insight card.
fn infer_sources(marker_id: &str, ctx: &PersonalizationContext) -> Vec<String> {
    match marker_id {
        "hardware_benchmark" => {
            let mut sources = vec!["Sovereign Profile".to_string()];
            if ctx.settings.has_llm {
                sources.push("LLM Config".into());
            }
            sources
        }
        "stack_fit" => vec!["Domain Profile".into(), "Tech Stack".into()],
        "cost_projection" => vec!["Regional Data".into(), "Sovereign Profile".into()],
        "t_shape" => vec!["Domain Profile".into(), "ACE Detection".into()],
        "engine_ranking" => vec![
            "Domain Profile".into(),
            "Sovereign Profile".into(),
            "Tech Radar".into(),
        ],
        "blind_spot_alert" => vec!["Developer DNA".into()],
        _ => vec!["Unknown".into()],
    }
}

/// Compute data coverage confidence for a given insight card.
fn compute_confidence(marker_id: &str, ctx: &PersonalizationContext) -> f64 {
    match marker_id {
        "hardware_benchmark" => {
            let mut score = 0.0;
            if !ctx.profile.cpu.is_empty() {
                score += 0.3;
            }
            if !ctx.profile.ram.is_empty() {
                score += 0.3;
            }
            if !ctx.profile.gpu.is_empty() {
                score += 0.4;
            }
            score
        }
        "stack_fit" | "engine_ranking" => {
            if ctx.stack.primary.len() >= 2 {
                0.9
            } else if !ctx.stack.primary.is_empty() {
                0.6
            } else {
                0.2
            }
        }
        "cost_projection" => {
            if ctx.regional.electricity_kwh > 0.0 {
                0.7
            } else {
                0.3
            }
        }
        "t_shape" => {
            if !ctx.stack.primary.is_empty() && !ctx.stack.adjacent.is_empty() {
                0.8
            } else if !ctx.stack.primary.is_empty() {
                0.5
            } else {
                0.1
            }
        }
        "blind_spot_alert" => {
            if ctx.dna.is_full {
                0.8
            } else {
                0.3
            }
        }
        _ => 0.5,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_personalization::context::*;

    fn test_ctx() -> PersonalizationContext {
        let mut cpu = std::collections::HashMap::new();
        cpu.insert("model".into(), "AMD Ryzen 9 7950X".into());
        cpu.insert("cores".into(), "16".into());

        let mut ram = std::collections::HashMap::new();
        ram.insert("total".into(), "64 GB".into());

        let mut gpu = std::collections::HashMap::new();
        gpu.insert("name".into(), "NVIDIA RTX 4090".into());
        gpu.insert("memory_total".into(), "24 GB".into());

        PersonalizationContext {
            profile: ProfileData {
                cpu,
                ram,
                gpu,
                ..Default::default()
            },
            stack: StackData {
                primary: vec!["rust".into(), "typescript".into()],
                adjacent: vec!["wasm".into(), "tauri".into(), "react".into()],
                ..Default::default()
            },
            radar: RadarData {
                adopt: vec!["rust".into()],
                trial: vec!["zig".into()],
                ..Default::default()
            },
            regional: RegionalData {
                currency: "USD".into(),
                currency_symbol: "$".into(),
                electricity_kwh: 0.16,
                internet_monthly: 70.0,
                ..Default::default()
            },
            decisions: Vec::new(),
            progress: ProgressData::default(),
            settings: SettingsData {
                has_llm: true,
                llm_provider: "ollama".into(),
                llm_model: "llama3".into(),
            },
            dna: DnaData {
                is_full: true,
                primary_stack: vec!["rust".into()],
                blind_spots: vec!["kubernetes".into(), "terraform".into()],
                top_engaged_topics: vec!["api".into(), "tooling".into()],
                ..Default::default()
            },
            computed: ComputedFields {
                llm_tier: "local".into(),
                gpu_tier: "workstation".into(),
                has_nvidia: true,
                os_family: "windows".into(),
                profile_completeness: 66.0,
                monthly_electricity_estimate: 48.0,
            },
        }
    }

    #[test]
    fn test_hardware_benchmark_card() {
        let ctx = test_ctx();
        let card = compute_hardware_benchmark(&ctx).unwrap();
        assert_eq!(card.card_type, CardType::HardwareBenchmark);
        assert!(card.data_points.iter().any(|dp| dp.label == "CPU"));
        assert!(card.data_points.iter().any(|dp| dp.value.contains("64 GB")));
        assert!(card.visualization.is_some());
    }

    #[test]
    fn test_hardware_benchmark_empty_profile() {
        let ctx = PersonalizationContext {
            profile: ProfileData::default(),
            stack: StackData::default(),
            radar: RadarData::default(),
            regional: RegionalData::default(),
            decisions: Vec::new(),
            progress: ProgressData::default(),
            settings: SettingsData::default(),
            dna: DnaData::default(),
            computed: ComputedFields::default(),
        };
        assert!(compute_hardware_benchmark(&ctx).is_none());
    }

    #[test]
    fn test_engine_ranking() {
        let ctx = test_ctx();
        let card = compute_engine_ranking(&ctx).unwrap();
        assert_eq!(card.card_type, CardType::EngineRanking);
        // Should have top 3 data points
        assert_eq!(card.data_points.len(), 3);
        // Visualization should be a rank list
        assert!(matches!(
            card.visualization,
            Some(Visualization::RankList { .. })
        ));
    }

    #[test]
    fn test_stack_fit_scoring() {
        let ctx = test_ctx();
        // Engine 5 (Dev Tools) requires rust/go/ts/python — user has rust+ts
        let score = engine_stack_score(&ENGINES[4], &ctx);
        assert!(score >= 0.5); // At least 2/4 match
    }

    #[test]
    fn test_cost_projection() {
        let ctx = test_ctx();
        let card = compute_cost_projection(&ctx).unwrap();
        assert_eq!(card.card_type, CardType::CostProjection);
        // Should include electricity and internet
        assert!(card
            .data_points
            .iter()
            .any(|dp| dp.label.contains("Electricity")));
    }

    #[test]
    fn test_t_shape() {
        let ctx = test_ctx();
        let card = compute_t_shape(&ctx).unwrap();
        assert!(matches!(
            card.visualization,
            Some(Visualization::TShape { .. })
        ));
    }

    #[test]
    fn test_parse_gb() {
        assert_eq!(parse_gb("32 GB"), 32.0);
        assert_eq!(parse_gb("16384 MB"), 16.0);
        assert_eq!(parse_gb("1 TB"), 1024.0);
        assert_eq!(parse_gb("32"), 32.0);
        assert_eq!(parse_gb(""), 0.0);
    }

    #[test]
    fn test_mirror_blocks() {
        let ctx = test_ctx();
        let blocks = compute_mirror_blocks(&ctx, "S", 0);
        // Should have at least blind_spot_moat and radar_momentum
        assert!(!blocks.is_empty());
        assert!(blocks.iter().any(|b| b.block_id == "blind_spot_moat"));
    }

    #[test]
    fn test_confidence_scoring() {
        let ctx = test_ctx();
        let conf = compute_confidence("hardware_benchmark", &ctx);
        assert!(conf >= 0.9); // Has CPU + RAM + GPU
    }
}

//! No-LLM Computation Engine — structured data cards from sovereign data.
//!
//! Produces `SovereignInsightCard` structs with data points and visualizations
//! computed entirely from local sovereign data. No LLM calls required.

use super::context::PersonalizationContext;
use super::engine_scoring::{
    compute_confidence, engine_hardware_score, engine_stack_score, infer_sources, parse_gb,
    ram_tier_context, ENGINES,
};
use super::{
    BarEntry, BlockPosition, CardType, DataPoint, InsightBlock, InsightContent, RankItem,
    SovereignInsightCard, Visualization,
};

use tracing::debug;

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
                format!("{cpu_model} ({cpu_cores} cores)")
            },
            context: None,
            highlight: false,
        },
        DataPoint {
            label: "RAM".into(),
            value: if ram_gb > 0.0 {
                format!("{ram_gb:.0} GB")
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
                Some(format!("{gpu_mem_gb:.0} GB VRAM"))
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
            label: format!("Engine {id}"),
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
            value: format!("{symbol}{electricity_monthly:.2}/mo"),
            context: Some(format!(
                "{:.1} kWh × {}{:.3}/kWh",
                ctx.computed.monthly_electricity_estimate, symbol, ctx.regional.electricity_kwh
            )),
            highlight: false,
        },
        DataPoint {
            label: "Internet".into(),
            value: format!("{symbol}{internet:.0}/mo"),
            context: None,
            highlight: false,
        },
        DataPoint {
            label: "Base Operating Cost".into(),
            value: format!("{symbol}{total:.2}/mo"),
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
            depth_label: format!("{primary} (primary)"),
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

// L4 mirror blocks extracted to mirror_blocks.rs — re-export for callers.
pub use super::mirror_blocks::compute_mirror_blocks;

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
}

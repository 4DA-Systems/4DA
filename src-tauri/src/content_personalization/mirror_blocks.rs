//! L4 Mirror / Sovereign Connection Blocks — cross-data correlations.
//!
//! Computes `MirrorBlock` structs that surface cross-data sovereign connections,
//! e.g. blind-spot-to-moat, feed-predicts-engine, and radar momentum signals.

use super::context::PersonalizationContext;
use super::engine_scoring::ENGINES;
use super::{ConnectionType, MirrorBlock};

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
        if matches > 0 && best_match.map(|(_, c)| matches > c).unwrap_or(true) {
            best_match = Some((engine.name, matches));
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_personalization::context::*;

    fn test_ctx() -> PersonalizationContext {
        PersonalizationContext {
            profile: ProfileData::default(),
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
            regional: RegionalData::default(),
            decisions: Vec::new(),
            progress: ProgressData::default(),
            settings: SettingsData::default(),
            dna: DnaData {
                is_full: true,
                primary_stack: vec!["rust".into()],
                blind_spots: vec!["kubernetes".into(), "terraform".into()],
                top_engaged_topics: vec!["api".into(), "tooling".into()],
                ..Default::default()
            },
            computed: ComputedFields::default(),
        }
    }

    #[test]
    fn test_mirror_blocks() {
        let ctx = test_ctx();
        let blocks = compute_mirror_blocks(&ctx, "S", 0);
        // Should have at least blind_spot_moat and radar_momentum
        assert!(!blocks.is_empty());
        assert!(blocks.iter().any(|b| b.block_id == "blind_spot_moat"));
    }
}

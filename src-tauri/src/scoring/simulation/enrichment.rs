// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Context Enrichment Engine — merges PersonaEnrichment into base ScoringContext.
//!
//! Takes a base `ScoringContext` (from `personas.rs`) and a `PersonaEnrichment`
//! (from `persona_data.rs`), returns a fully-populated `ScoringContext` with all
//! 18 active fields set. Supports per-signal toggle for isolation testing.

use super::super::ace_context::ACEContext;
use super::super::ScoringContext;
use super::persona_data::PersonaEnrichment;

// ============================================================================
// EnrichmentConfig — per-signal toggle
// ============================================================================

/// Controls which enrichment signals are applied.
/// Enables signal isolation testing: enable one field at a time to measure
/// its individual contribution to scoring quality.
pub(super) struct EnrichmentConfig {
    pub enable_topic_confidence: bool,
    pub enable_topic_affinities: bool,
    pub enable_anti_topics: bool,
    pub enable_topic_embeddings: bool,
    pub enable_source_quality: bool,
    pub enable_work_topics: bool,
    pub enable_calibration_deltas: bool,
    pub enable_taste_embedding: bool,
    pub enable_topic_half_lives: bool,
    pub enable_exclusions: bool,
    pub enable_open_windows: bool,
    pub enable_sovereign_profile: bool,
    pub enable_dependency_info: bool,
}

impl EnrichmentConfig {
    /// All fields enabled — full-fidelity simulation.
    pub fn all() -> Self {
        Self {
            enable_topic_confidence: true,
            enable_topic_affinities: true,
            enable_anti_topics: true,
            enable_topic_embeddings: true,
            enable_source_quality: true,
            enable_work_topics: true,
            enable_calibration_deltas: true,
            enable_taste_embedding: true,
            enable_topic_half_lives: true,
            enable_exclusions: true,
            enable_open_windows: true,
            enable_sovereign_profile: true,
            enable_dependency_info: true,
        }
    }

    /// All fields disabled — legacy behavior matching current simulation.
    pub fn none() -> Self {
        Self {
            enable_topic_confidence: false,
            enable_topic_affinities: false,
            enable_anti_topics: false,
            enable_topic_embeddings: false,
            enable_source_quality: false,
            enable_work_topics: false,
            enable_calibration_deltas: false,
            enable_taste_embedding: false,
            enable_topic_half_lives: false,
            enable_exclusions: false,
            enable_open_windows: false,
            enable_sovereign_profile: false,
            enable_dependency_info: false,
        }
    }

    /// Single field enabled — for signal isolation testing.
    pub fn only(field: EnrichmentField) -> Self {
        let mut cfg = Self::none();
        match field {
            EnrichmentField::TopicConfidence => cfg.enable_topic_confidence = true,
            EnrichmentField::TopicAffinities => cfg.enable_topic_affinities = true,
            EnrichmentField::AntiTopics => cfg.enable_anti_topics = true,
            EnrichmentField::TopicEmbeddings => cfg.enable_topic_embeddings = true,
            EnrichmentField::SourceQuality => cfg.enable_source_quality = true,
            EnrichmentField::WorkTopics => cfg.enable_work_topics = true,
            EnrichmentField::CalibrationDeltas => cfg.enable_calibration_deltas = true,
            EnrichmentField::TasteEmbedding => cfg.enable_taste_embedding = true,
            EnrichmentField::TopicHalfLives => cfg.enable_topic_half_lives = true,
            EnrichmentField::Exclusions => cfg.enable_exclusions = true,
            EnrichmentField::OpenWindows => cfg.enable_open_windows = true,
            EnrichmentField::SovereignProfile => cfg.enable_sovereign_profile = true,
            EnrichmentField::DependencyInfo => cfg.enable_dependency_info = true,
        }
        cfg
    }
}

/// Enumeration of enrichable fields, for use with `EnrichmentConfig::only()`.
#[derive(Debug, Clone, Copy)]
pub(super) enum EnrichmentField {
    TopicConfidence,
    TopicAffinities,
    AntiTopics,
    TopicEmbeddings,
    SourceQuality,
    WorkTopics,
    CalibrationDeltas,
    TasteEmbedding,
    TopicHalfLives,
    Exclusions,
    OpenWindows,
    SovereignProfile,
    DependencyInfo,
}

impl EnrichmentField {
    /// All field variants for iteration.
    pub fn all_variants() -> &'static [EnrichmentField] {
        &[
            EnrichmentField::TopicConfidence,
            EnrichmentField::TopicAffinities,
            EnrichmentField::AntiTopics,
            EnrichmentField::TopicEmbeddings,
            EnrichmentField::SourceQuality,
            EnrichmentField::WorkTopics,
            EnrichmentField::CalibrationDeltas,
            EnrichmentField::TasteEmbedding,
            EnrichmentField::TopicHalfLives,
            EnrichmentField::Exclusions,
            EnrichmentField::OpenWindows,
            EnrichmentField::SovereignProfile,
            EnrichmentField::DependencyInfo,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::TopicConfidence => "topic_confidence",
            Self::TopicAffinities => "topic_affinities",
            Self::AntiTopics => "anti_topics",
            Self::TopicEmbeddings => "topic_embeddings",
            Self::SourceQuality => "source_quality",
            Self::WorkTopics => "work_topics",
            Self::CalibrationDeltas => "calibration_deltas",
            Self::TasteEmbedding => "taste_embedding",
            Self::TopicHalfLives => "topic_half_lives",
            Self::Exclusions => "exclusions",
            Self::OpenWindows => "open_windows",
            Self::SovereignProfile => "sovereign_profile",
            Self::DependencyInfo => "dependency_info",
        }
    }
}

// ============================================================================
// Core enrichment function
// ============================================================================

/// Enrich a base ScoringContext with PersonaEnrichment data.
///
/// Merges enrichment fields into the base context according to config toggles.
/// Base fields (interests, domain_profile, declared_tech, composed_stack,
/// feedback_boosts, feedback_interaction_count) are preserved from the original.
pub(super) fn enrich_persona(
    base: ScoringContext,
    data: &PersonaEnrichment,
    config: &EnrichmentConfig,
) -> ScoringContext {
    // 1. Build enriched ACEContext from the base
    let mut ace = ACEContext {
        active_topics: base.ace_ctx.active_topics.clone(),
        topic_confidence: base.ace_ctx.topic_confidence.clone(),
        detected_tech: base.ace_ctx.detected_tech.clone(),
        anti_topics: base.ace_ctx.anti_topics.clone(),
        anti_topic_confidence: base.ace_ctx.anti_topic_confidence.clone(),
        topic_affinities: base.ace_ctx.topic_affinities.clone(),
        dependency_names: base.ace_ctx.dependency_names.clone(),
        dependency_info: base.ace_ctx.dependency_info.clone(),
        peak_hours: base.ace_ctx.peak_hours.clone(),
        tech_weights: base.ace_ctx.tech_weights.clone(),
        negative_stack: base.ace_ctx.negative_stack.clone(),
    };

    // 2. Merge ACEContext enrichment fields
    if config.enable_topic_confidence {
        for (topic, &conf) in &data.topic_confidence {
            ace.topic_confidence
                .entry(topic.clone())
                .and_modify(|existing| *existing = existing.max(conf))
                .or_insert(conf);
        }
    }

    if config.enable_topic_affinities {
        for (topic, &(aff, conf)) in &data.topic_affinities {
            ace.topic_affinities
                .entry(topic.clone())
                .or_insert((aff, conf));
        }
    }

    if config.enable_anti_topics {
        for anti in &data.anti_topics {
            if !ace.anti_topics.contains(anti) {
                ace.anti_topics.push(anti.clone());
            }
        }
        for (topic, &conf) in &data.anti_topic_confidence {
            ace.anti_topic_confidence
                .entry(topic.clone())
                .or_insert(conf);
        }
    }

    if config.enable_dependency_info {
        for dep in &data.dependency_info {
            let name = dep.package_name.to_string();
            ace.dependency_info.entry(name.clone()).or_insert_with(|| {
                crate::scoring::dependencies::DepInfo {
                    package_name: name.clone(),
                    version: dep.version.map(|v| v.to_string()),
                    is_dev: dep.is_dev,
                    is_direct: dep.is_direct,
                    search_terms: dep.search_terms.iter().map(|s| s.to_string()).collect(),
                }
            });
        }
        for name in &data.dependency_names {
            ace.dependency_names.insert(name.clone());
        }
    }

    // 3. Build enriched top-level fields
    let topic_embeddings = if config.enable_topic_embeddings {
        data.topic_embeddings.clone()
    } else {
        base.topic_embeddings
    };

    let source_quality = if config.enable_source_quality {
        data.source_quality.clone()
    } else {
        base.source_quality
    };

    let work_topics = if config.enable_work_topics {
        data.work_topics.clone()
    } else {
        base.work_topics
    };

    let calibration_deltas = if config.enable_calibration_deltas {
        data.calibration_deltas.clone()
    } else {
        base.calibration_deltas
    };

    let taste_embedding = if config.enable_taste_embedding {
        data.taste_embedding.clone()
    } else {
        base.taste_embedding
    };

    let topic_half_lives = if config.enable_topic_half_lives {
        data.topic_half_lives.clone()
    } else {
        base.topic_half_lives
    };

    let exclusions = if config.enable_exclusions {
        data.exclusions.clone()
    } else {
        base.exclusions
    };

    let open_windows = if config.enable_open_windows {
        data.open_windows
            .iter()
            .map(|w| crate::decision_advantage::DecisionWindow {
                id: 0,
                window_type: w.window_type.to_string(),
                title: w.title.to_string(),
                description: String::new(),
                urgency: w.urgency,
                relevance: w.relevance,
                dependency: w.dependency.map(|d| d.to_string()),
                status: "open".to_string(),
                opened_at: String::new(),
                expires_at: None,
                lead_time_hours: None,
                streets_engine: None,
            })
            .collect()
    } else {
        base.open_windows
    };

    let sovereign_profile = if config.enable_sovereign_profile && !data.skill_gaps.is_empty() {
        Some(build_minimal_sovereign_profile(&data.skill_gaps))
    } else {
        base.sovereign_profile
    };

    // 4. Rebuild ScoringContext via builder
    ScoringContext::builder()
        .cached_context_count(base.cached_context_count)
        .interest_count(base.interest_count)
        .interests(base.interests)
        .exclusions(exclusions)
        .ace_ctx(ace)
        .topic_embeddings(topic_embeddings)
        .feedback_boosts(base.feedback_boosts)
        .source_quality(source_quality)
        .declared_tech(base.declared_tech)
        .domain_profile(base.domain_profile)
        .work_topics(work_topics)
        .feedback_interaction_count(base.feedback_interaction_count)
        .composed_stack(base.composed_stack)
        .open_windows(open_windows)
        .calibration_deltas(calibration_deltas)
        .taste_embedding(taste_embedding)
        .topic_half_lives(topic_half_lives)
        .sovereign_profile(sovereign_profile)
        .build()
}

// ============================================================================
// Helpers
// ============================================================================

fn build_minimal_sovereign_profile(
    skill_gaps: &[(&str, &str)],
) -> crate::sovereign_developer_profile::SovereignDeveloperProfile {
    use crate::sovereign_developer_profile::*;
    SovereignDeveloperProfile {
        generated_at: "simulation".to_string(),
        identity_summary: "simulation persona".to_string(),
        infrastructure: InfrastructureDimension::default(),
        stack: StackDimension::default(),
        skills: SkillsDimension::default(),
        preferences: PreferencesDimension::default(),
        context: ContextDimension::default(),
        intelligence: IntelligenceReport {
            skill_gaps: skill_gaps
                .iter()
                .map(|(dep, reason)| SkillGap {
                    dependency: dep.to_string(),
                    reason: reason.to_string(),
                })
                .collect(),
            optimization_opportunities: vec![],
            infrastructure_mismatches: vec![],
            ecosystem_alerts: vec![],
        },
        completeness: CompletenessReport::default(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::persona_data::all_enrichments;
    use super::super::personas::all_personas;
    use super::*;

    #[test]
    fn enrichment_all_populates_fields() {
        let bases = all_personas();
        let enrichments = all_enrichments();
        let config = EnrichmentConfig::all();

        for (i, (base, data)) in bases.into_iter().zip(enrichments.iter()).enumerate() {
            let enriched = enrich_persona(base, data, &config);

            // topic_embeddings should be populated (except bootstrap which has 1)
            if i != 5 {
                assert!(
                    enriched.topic_embeddings.len() >= 2,
                    "Persona {i}: topic_embeddings not enriched"
                );
            }

            // ACE topic_confidence should have entries if data has them
            if !data.topic_confidence.is_empty() {
                assert!(
                    !enriched.ace_ctx.topic_confidence.is_empty(),
                    "Persona {i}: topic_confidence not enriched"
                );
            }

            // Anti-topics should be set if data has them
            if !data.anti_topics.is_empty() {
                assert!(
                    !enriched.ace_ctx.anti_topics.is_empty(),
                    "Persona {i}: anti_topics not enriched"
                );
            }
        }
    }

    #[test]
    fn enrichment_none_preserves_base() {
        let bases = all_personas();
        let enrichments = all_enrichments();
        let config = EnrichmentConfig::none();

        // For a non-trivial persona (Rust), enriching with none() should leave
        // top-level fields at their defaults (empty)
        let enriched = enrich_persona(
            bases.into_iter().next().expect("at least one persona"),
            &enrichments[0],
            &config,
        );
        assert!(
            enriched.topic_embeddings.is_empty(),
            "EnrichmentConfig::none() should not add topic_embeddings"
        );
        assert!(
            enriched.source_quality.is_empty(),
            "EnrichmentConfig::none() should not add source_quality"
        );
        assert!(
            enriched.work_topics.is_empty(),
            "EnrichmentConfig::none() should not add work_topics"
        );
        assert!(
            enriched.exclusions.is_empty(),
            "EnrichmentConfig::none() should not add exclusions"
        );
    }

    #[test]
    fn enrichment_only_enables_single_field() {
        let bases = all_personas();
        let enrichments = all_enrichments();
        let config = EnrichmentConfig::only(EnrichmentField::SourceQuality);

        let enriched = enrich_persona(
            bases.into_iter().next().expect("at least one persona"),
            &enrichments[0],
            &config,
        );
        // Source quality should be set
        assert!(
            !enriched.source_quality.is_empty(),
            "only(SourceQuality) should set source_quality"
        );
        // But topic_embeddings should still be empty
        assert!(
            enriched.topic_embeddings.is_empty(),
            "only(SourceQuality) should not set topic_embeddings"
        );
        // And exclusions should still be empty
        assert!(
            enriched.exclusions.is_empty(),
            "only(SourceQuality) should not set exclusions"
        );
    }
}

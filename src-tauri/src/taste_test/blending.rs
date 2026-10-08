// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Persona blending — converts posterior persona weights into production context.
//!
//! Maps each persona to characteristic interests, tech stack, and exclusions,
//! then blends them according to the inferred weights.

use std::collections::{HashMap, HashSet};

use super::items::CARD_TOPICS;
use super::TasteResponse;

// ============================================================================
// Persona Templates
// ============================================================================

pub(crate) struct PersonaTemplate {
    pub interests: &'static [(&'static str, f32)],
    pub tech: &'static [&'static str],
    pub exclusions: &'static [&'static str],
    pub stack_ids: &'static [&'static str],
}

/// Templates for each persona in canonical order.
pub(crate) static TEMPLATES: [PersonaTemplate; 9] = [
    // 0: rust_systems
    PersonaTemplate {
        interests: &[
            ("Rust", 1.0),
            ("systems programming", 0.9),
            ("Tauri", 0.8),
            ("async runtime", 0.7),
            ("WebAssembly", 0.7),
            ("embedded systems", 0.6),
            ("memory safety", 0.8),
            ("performance", 0.7),
        ],
        tech: &["rust", "tauri", "sqlite", "tokio", "wasm"],
        exclusions: &[],
        stack_ids: &["rust_systems"],
    },
    // 1: python_ml
    PersonaTemplate {
        interests: &[
            ("Machine Learning", 1.0),
            ("PyTorch", 0.9),
            ("deep learning", 0.9),
            ("data science", 0.8),
            ("AI/LLM", 0.9),
            ("Python", 0.7),
            ("computer vision", 0.6),
            ("NLP", 0.7),
        ],
        tech: &["python", "pytorch", "tensorflow", "jupyter", "numpy"],
        exclusions: &[],
        stack_ids: &["python_ml"],
    },
    // 2: fullstack_ts
    PersonaTemplate {
        interests: &[
            ("TypeScript", 1.0),
            ("React", 0.9),
            ("Next.js", 0.9),
            ("Web Development", 0.8),
            ("Node.js", 0.7),
            ("frontend", 0.7),
            ("CSS", 0.5),
            ("full-stack", 0.8),
        ],
        tech: &["typescript", "react", "nextjs", "nodejs", "tailwind"],
        exclusions: &[],
        stack_ids: &["nextjs_fullstack"],
    },
    // 3: devops_sre
    PersonaTemplate {
        interests: &[
            ("Kubernetes", 1.0),
            ("DevOps", 0.9),
            ("cloud infrastructure", 0.9),
            ("observability", 0.8),
            ("CI/CD", 0.7),
            ("SRE", 0.8),
            ("Docker", 0.7),
            ("Terraform", 0.7),
        ],
        tech: &["kubernetes", "docker", "terraform", "aws", "prometheus"],
        exclusions: &[],
        stack_ids: &["devops_sre"],
    },
    // 4: mobile_dev
    PersonaTemplate {
        interests: &[
            ("Mobile Development", 1.0),
            ("React Native", 0.9),
            ("iOS", 0.8),
            ("Android", 0.8),
            ("cross-platform", 0.7),
            ("Swift", 0.6),
            ("Kotlin", 0.6),
            ("mobile UX", 0.7),
        ],
        tech: &["react-native", "swift", "kotlin", "expo", "flutter"],
        exclusions: &[],
        stack_ids: &["mobile_dev"],
    },
    // 5: bootstrap
    PersonaTemplate {
        interests: &[
            ("Startups", 0.9),
            ("product development", 0.8),
            ("rapid prototyping", 0.8),
            ("SaaS", 0.7),
            ("indie hacking", 0.7),
            ("no-code", 0.5),
            ("MVP", 0.8),
            ("growth", 0.6),
        ],
        tech: &["nextjs", "vercel", "supabase", "stripe", "tailwind"],
        exclusions: &[],
        stack_ids: &["bootstrap_builder"],
    },
    // 6: power_user
    PersonaTemplate {
        interests: &[
            ("Open Source", 0.8),
            ("programming languages", 0.7),
            ("databases", 0.7),
            ("distributed systems", 0.8),
            ("security", 0.6),
            ("compilers", 0.6),
            ("algorithms", 0.6),
            ("systems design", 0.8),
        ],
        tech: &["rust", "go", "python", "typescript", "linux"],
        exclusions: &[],
        stack_ids: &["power_user"],
    },
    // 7: context_switcher
    PersonaTemplate {
        interests: &[
            ("Go", 0.8),
            ("microservices", 0.7),
            ("API design", 0.7),
            ("cloud native", 0.7),
            ("DevOps", 0.6),
            ("backend", 0.7),
            ("architecture", 0.7),
            ("pragmatic engineering", 0.6),
        ],
        tech: &["go", "docker", "grpc", "postgresql", "redis"],
        exclusions: &[],
        stack_ids: &["context_switcher"],
    },
    // 8: niche_specialist
    PersonaTemplate {
        interests: &[
            ("Haskell", 0.9),
            ("functional programming", 0.9),
            ("type theory", 0.8),
            ("category theory", 0.6),
            ("Erlang", 0.5),
            ("formal verification", 0.6),
            ("PLT", 0.7),
            ("academic CS", 0.5),
        ],
        tech: &["haskell", "ocaml", "agda", "coq", "erlang"],
        exclusions: &[],
        stack_ids: &["niche_specialist"],
    },
];

// ============================================================================
// Blended Profile
// ============================================================================

/// A persona's TEMPLATE topics are added to the profile only when its
/// posterior is at least this high. Below it, a template topic is a guess
/// about a persona the user may not be — the fresh-profile audit (2026-10-07)
/// saw "Tauri" and "embedded systems" written as real interests for a user
/// who liked only React / React Native cards.
pub(crate) const PERSONA_TOPIC_MIN_POSTERIOR: f64 = 0.5;

/// Ceiling on a template topic's weight. Template topics are inferred, never
/// stated, so they always rank below the topics of cards the user liked.
pub(crate) const INFERRED_TOPIC_WEIGHT_CAP: f32 = 0.5;

/// Result of blending persona weights into a production-ready context.
#[derive(Debug, Clone)]
pub struct BlendedProfile {
    /// Interest topics: the topics of LIKED cards (normalized to [0, 1],
    /// descending) followed by persona-inferred template topics (weight
    /// <= [`INFERRED_TOPIC_WEIGHT_CAP`], descending).
    pub interests: Vec<(String, f32)>,
    /// The entries of `interests` that came from a persona template rather
    /// than from a liked card — the "inferred" tag.
    pub inferred_topics: Vec<String>,
    /// Union of tech stack items from contributing personas.
    pub tech_stack: Vec<String>,
    /// Anti-topics from dominant persona only.
    pub exclusions: Vec<String>,
    /// Stack profile IDs for compose_profiles().
    pub stack_ids: Vec<String>,
    /// Per-topic scoring corrections.
    pub calibration_deltas: HashMap<String, f32>,
}

fn dominant_persona(weights: &[f64; 9]) -> usize {
    weights
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i)
}

fn sorted_desc(map: HashMap<String, f32>) -> Vec<(String, f32)> {
    let mut v: Vec<(String, f32)> = map.into_iter().collect();
    v.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    v
}

fn card_topics(slot: usize) -> &'static [(&'static str, f32)] {
    CARD_TOPICS.get(slot).copied().unwrap_or(&[])
}

/// Topics of the cards the user liked, weighted by card-topic weight (a
/// "love" counts fully, a plain "interested" at 0.8), normalized to [0, 1].
fn liked_card_topics(responses: &[(usize, TasteResponse)]) -> Vec<(String, f32)> {
    let mut acc: HashMap<String, f32> = HashMap::new();
    for (slot, response) in responses {
        let factor = match response {
            TasteResponse::StrongInterest => 1.0,
            TasteResponse::Interested => 0.8,
            TasteResponse::NotInterested => continue,
        };
        for &(topic, w) in card_topics(*slot) {
            *acc.entry(topic.to_string()).or_insert(0.0) += w * factor;
        }
    }
    let max = acc.values().copied().fold(0.0f32, f32::max);
    if max > 0.0 {
        for w in acc.values_mut() {
            *w /= max;
        }
    }
    sorted_desc(acc)
}

/// Template topics of every persona whose posterior clears
/// [`PERSONA_TOPIC_MIN_POSTERIOR`], minus topics already liked and topics the
/// user explicitly passed on (on a skipped card and no liked one).
fn persona_template_topics(
    weights: &[f64; 9],
    responses: &[(usize, TasteResponse)],
    liked: &[(String, f32)],
) -> Vec<(String, f32)> {
    let liked_set: HashSet<String> = liked.iter().map(|(t, _)| t.to_lowercase()).collect();
    let passed: HashSet<String> = responses
        .iter()
        .filter(|(_, r)| matches!(r, TasteResponse::NotInterested))
        .flat_map(|(slot, _)| card_topics(*slot))
        .map(|(t, _)| t.to_lowercase())
        .filter(|t| !liked_set.contains(t))
        .collect();

    let mut acc: HashMap<String, f32> = HashMap::new();
    for (i, &w) in weights.iter().enumerate() {
        if w < PERSONA_TOPIC_MIN_POSTERIOR {
            continue;
        }
        for &(topic, tw) in TEMPLATES[i].interests {
            let key = topic.to_lowercase();
            if liked_set.contains(&key) || passed.contains(&key) {
                continue;
            }
            let weight = INFERRED_TOPIC_WEIGHT_CAP * w as f32 * tw;
            let entry = acc.entry(topic.to_string()).or_insert(0.0);
            *entry = entry.max(weight);
        }
    }
    sorted_desc(acc)
}

/// Union of tech + stack ids over above-threshold personas.
fn template_union(weights: &[f64; 9], threshold: f64) -> (Vec<String>, Vec<String>) {
    let mut tech_set: Vec<String> = Vec::new();
    let mut stack_set: Vec<String> = Vec::new();
    for (i, &w) in weights.iter().enumerate() {
        if w < threshold {
            continue;
        }
        for &tech in TEMPLATES[i].tech {
            if !tech_set.iter().any(|t| t == tech) {
                tech_set.push(tech.to_string());
            }
        }
        for &sid in TEMPLATES[i].stack_ids {
            if !stack_set.iter().any(|s| s == sid) {
                stack_set.push(sid.to_string());
            }
        }
    }
    (tech_set, stack_set)
}

/// Calibration deltas: topics from non-dominant personas get a positive delta
/// (boosting their relevance slightly since the user showed interest).
fn calibration_deltas(weights: &[f64; 9], threshold: f64, dominant: usize) -> HashMap<String, f32> {
    let mut deltas: HashMap<String, f32> = HashMap::new();
    let dominant_weight = weights[dominant];
    for (i, &w) in weights.iter().enumerate() {
        if i == dominant || w < threshold {
            continue;
        }
        for &(topic, _) in TEMPLATES[i].interests {
            let delta = (w / dominant_weight) as f32 * 0.15;
            deltas
                .entry(topic.to_string())
                .and_modify(|d| *d = d.max(delta))
                .or_insert(delta);
        }
    }
    deltas
}

/// Blend persona weights and the user's actual answers into a unified profile.
///
/// # Arguments
/// - `weights`: Posterior probability for each of the 9 personas
/// - `threshold`: Minimum weight to contribute tech / stack ids / deltas
///   (typically 0.10)
/// - `responses`: The `(slot, response)` answers. Interests come from the
///   topics of the LIKED cards; persona template topics are appended (tagged
///   inferred, lower weight) only for personas at or above
///   [`PERSONA_TOPIC_MIN_POSTERIOR`].
pub fn blend_profile(
    weights: &[f64; 9],
    threshold: f64,
    responses: &[(usize, TasteResponse)],
) -> BlendedProfile {
    let dominant = dominant_persona(weights);
    let liked = liked_card_topics(responses);
    let inferred = persona_template_topics(weights, responses, &liked);
    let inferred_topics = inferred.iter().map(|(t, _)| t.clone()).collect();
    let mut interests = liked;
    interests.extend(inferred);

    let (tech_stack, stack_ids) = template_union(weights, threshold);
    let exclusions: Vec<String> = TEMPLATES[dominant]
        .exclusions
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    BlendedProfile {
        interests,
        inferred_topics,
        tech_stack,
        exclusions,
        stack_ids,
        calibration_deltas: calibration_deltas(weights, threshold, dominant),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taste_test::inference::InferenceState;

    fn make_weights(dominant: usize) -> [f64; 9] {
        let mut w = [0.0; 9];
        w[dominant] = 1.0;
        w
    }

    fn has(profile: &BlendedProfile, topic: &str) -> bool {
        profile.interests.iter().any(|(t, _)| t == topic)
    }

    fn topic_names(interests: &[(String, f32)]) -> Vec<&str> {
        interests.iter().map(|(t, _)| t.as_str()).collect()
    }

    #[test]
    fn test_pure_rust_persona_blend() {
        let profile = blend_profile(&make_weights(0), 0.10, &[]);
        assert!(has(&profile, "Rust"), "Rust should be in interests");
        assert!(profile.inferred_topics.iter().any(|t| t == "Rust"));
        assert!(profile.tech_stack.contains(&"rust".to_string()));
    }

    #[test]
    fn even_blend_invents_no_interests() {
        // No persona clears the posterior cutoff and nothing was liked:
        // there is no evidence for any interest, so none is written.
        let weights = [1.0 / 9.0; 9];
        let profile = blend_profile(&weights, 0.10, &[]);
        assert!(profile.interests.is_empty(), "{:?}", profile.interests);
        assert!(profile.tech_stack.len() > 10, "tech union still blends");
    }

    #[test]
    fn test_threshold_filtering() {
        let mut weights = [0.0; 9];
        weights[0] = 0.95;
        weights[1] = 0.05; // Below threshold
        let profile = blend_profile(&weights, 0.10, &[]);
        assert!(
            !has(&profile, "PyTorch"),
            "Below-threshold personas should not contribute interests"
        );
    }

    #[test]
    fn test_exclusions_from_dominant_only() {
        let profile = blend_profile(&make_weights(0), 0.10, &[]);
        assert!(profile.exclusions.len() == TEMPLATES[0].exclusions.len());
    }

    #[test]
    fn test_calibration_deltas_computed() {
        let mut weights = [0.0; 9];
        weights[0] = 0.60;
        weights[1] = 0.25;
        weights[2] = 0.15;
        let profile = blend_profile(&weights, 0.10, &[]);
        assert!(
            !profile.calibration_deltas.is_empty(),
            "Should have calibration deltas from non-dominant personas"
        );
    }

    #[test]
    fn liked_topics_are_normalized_and_outrank_inferred_ones() {
        let responses = [(0usize, TasteResponse::StrongInterest)];
        let profile = blend_profile(&make_weights(0), 0.10, &responses);
        assert_eq!(profile.interests[0], ("Rust".to_string(), 1.0));
        for (topic, weight) in &profile.interests {
            assert!((0.0..=1.0).contains(weight), "{topic} = {weight}");
        }
        for (topic, weight) in profile.interests.iter().skip(1) {
            assert!(profile.inferred_topics.contains(topic));
            assert!(*weight <= INFERRED_TOPIC_WEIGHT_CAP, "{topic} = {weight}");
        }
        assert!(
            !profile.inferred_topics.iter().any(|t| t == "Rust"),
            "a liked topic is not tagged inferred"
        );
    }

    #[test]
    fn passed_card_topics_are_never_inferred() {
        // Rust persona is certain, but the user skipped the WebAssembly card.
        let responses = [(8usize, TasteResponse::NotInterested)];
        let profile = blend_profile(&make_weights(0), 0.10, &responses);
        assert!(!has(&profile, "WebAssembly"), "{:?}", profile.interests);
    }

    /// Audit 2026-10-07 repro: a React / React Native user. The profile must
    /// name what they liked, and nothing from the Rust template.
    #[test]
    fn liking_react_native_and_nextjs_yields_react_not_tauri() {
        let mut state = InferenceState::new();
        state.update(4, &TasteResponse::Interested); // React Native
        state.update(3, &TasteResponse::Interested); // Next.js
        for slot in [0usize, 1, 2, 5, 6, 7] {
            state.update(slot, &TasteResponse::NotInterested);
        }
        let profile = state.finalize();
        let topics = topic_names(&profile.inferred_interests);
        assert!(topics.contains(&"React Native"), "{topics:?}");
        assert!(topics.contains(&"React"), "{topics:?}");
        assert!(!topics.contains(&"Tauri"), "{topics:?}");
        assert!(!topics.contains(&"embedded systems"), "{topics:?}");
        let summary = state.build_summary();
        assert!(summary.top_interests.iter().any(|t| t == "React Native"));
        assert!(summary.top_interests.iter().any(|t| t == "React"));
    }

    #[test]
    fn one_rust_like_does_not_add_embedded_systems() {
        let mut state = InferenceState::new();
        state.update(0, &TasteResponse::Interested);
        let profile = state.finalize();
        assert!(
            profile.persona_weights[0] < PERSONA_TOPIC_MIN_POSTERIOR,
            "one like must leave the Rust persona below the cutoff"
        );
        assert_eq!(topic_names(&profile.inferred_interests), vec!["Rust"]);
    }
}

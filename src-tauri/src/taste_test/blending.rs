// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Persona blending — converts posterior persona weights into production context.
//!
//! Maps each persona to characteristic interests, tech stack, and exclusions,
//! then blends them according to the inferred weights.

use std::collections::HashMap;

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

/// Result of blending persona weights into a production-ready context.
#[derive(Debug, Clone)]
pub struct BlendedProfile {
    /// Interest topics with blended weights, sorted by weight descending.
    pub interests: Vec<(String, f32)>,
    /// Union of tech stack items from contributing personas.
    pub tech_stack: Vec<String>,
    /// Anti-topics from dominant persona only.
    pub exclusions: Vec<String>,
    /// Stack profile IDs for compose_profiles().
    pub stack_ids: Vec<String>,
    /// Per-topic scoring corrections.
    pub calibration_deltas: HashMap<String, f32>,
}

/// Blend persona weights into a unified profile.
///
/// # Arguments
/// - `weights`: Posterior probability for each of the 9 personas
/// - `threshold`: Minimum weight to contribute (typically 0.10)
pub fn blend_profile(weights: &[f64; 9], threshold: f64) -> BlendedProfile {
    let mut interest_weights: HashMap<String, f32> = HashMap::new();
    let mut tech_set: Vec<String> = Vec::new();
    let mut stack_set: Vec<String> = Vec::new();

    // Find dominant persona
    let dominant = weights
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i);

    // Blend interests from all above-threshold personas
    for (i, &w) in weights.iter().enumerate() {
        if w < threshold {
            continue;
        }

        let template = &TEMPLATES[i];

        // Weighted interest contribution
        for &(topic, topic_weight) in template.interests {
            let contribution = w as f32 * topic_weight;
            *interest_weights.entry(topic.to_string()).or_insert(0.0) += contribution;
        }

        // Tech stack union
        for &tech in template.tech {
            let t = tech.to_string();
            if !tech_set.contains(&t) {
                tech_set.push(t);
            }
        }

        // Stack IDs union
        for &sid in template.stack_ids {
            let s = sid.to_string();
            if !stack_set.contains(&s) {
                stack_set.push(s);
            }
        }
    }

    // Normalize interest weights to [0, 1]
    let max_weight = interest_weights.values().copied().fold(0.0f32, f32::max);
    if max_weight > 0.0 {
        for w in interest_weights.values_mut() {
            *w /= max_weight;
        }
    }

    // Sort by weight descending
    let mut interests: Vec<(String, f32)> = interest_weights.into_iter().collect();
    interests.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Exclusions from dominant persona only
    let exclusions: Vec<String> = TEMPLATES[dominant]
        .exclusions
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    // Calibration deltas: topics from non-dominant personas get a positive delta
    // (boosting their relevance slightly since the user showed interest)
    let mut calibration_deltas: HashMap<String, f32> = HashMap::new();
    let dominant_weight = weights[dominant];
    for (i, &w) in weights.iter().enumerate() {
        if i == dominant || w < threshold {
            continue;
        }
        for &(topic, _) in TEMPLATES[i].interests {
            let delta = (w / dominant_weight) as f32 * 0.15;
            calibration_deltas
                .entry(topic.to_string())
                .and_modify(|d| *d = d.max(delta))
                .or_insert(delta);
        }
    }

    BlendedProfile {
        interests,
        tech_stack: tech_set,
        exclusions,
        stack_ids: stack_set,
        calibration_deltas,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_weights(dominant: usize) -> [f64; 9] {
        let mut w = [0.0; 9];
        w[dominant] = 1.0;
        w
    }

    #[test]
    fn test_pure_rust_persona_blend() {
        let profile = blend_profile(&make_weights(0), 0.10);
        assert!(
            profile.interests.iter().any(|(t, _)| t == "Rust"),
            "Rust should be in interests"
        );
        assert!(profile.tech_stack.contains(&"rust".to_string()));
    }

    #[test]
    fn test_even_blend_union_interests() {
        let weights = [1.0 / 9.0; 9];
        let profile = blend_profile(&weights, 0.10);
        // Should have interests from multiple personas
        assert!(
            profile.interests.len() > 10,
            "Even blend should have many interests"
        );
        assert!(
            profile.tech_stack.len() > 10,
            "Even blend should have many tech"
        );
    }

    #[test]
    fn test_threshold_filtering() {
        let mut weights = [0.0; 9];
        weights[0] = 0.95;
        weights[1] = 0.05; // Below threshold
        let profile = blend_profile(&weights, 0.10);
        // Python ML interests should NOT appear (weight 0.05 < threshold 0.10)
        assert!(
            !profile.interests.iter().any(|(t, _)| t == "PyTorch"),
            "Below-threshold personas should not contribute interests"
        );
    }

    #[test]
    fn test_exclusions_from_dominant_only() {
        // All personas currently have empty exclusions, so test the mechanism
        let profile = blend_profile(&make_weights(0), 0.10);
        // Just verify it returns without error and exclusions is a valid vec
        assert!(profile.exclusions.len() == TEMPLATES[0].exclusions.len());
    }

    #[test]
    fn test_calibration_deltas_computed() {
        let mut weights = [0.0; 9];
        weights[0] = 0.60;
        weights[1] = 0.25;
        weights[2] = 0.15;
        let profile = blend_profile(&weights, 0.10);
        // Non-dominant personas should generate calibration deltas
        assert!(
            !profile.calibration_deltas.is_empty(),
            "Should have calibration deltas from non-dominant personas"
        );
    }

    #[test]
    fn test_interests_normalized() {
        let profile = blend_profile(&make_weights(0), 0.10);
        for (topic, weight) in &profile.interests {
            assert!(
                *weight >= 0.0 && *weight <= 1.0,
                "Interest '{}' weight {} should be in [0, 1]",
                topic,
                weight
            );
        }
        // At least one should be 1.0 (the max)
        assert!(
            profile
                .interests
                .iter()
                .any(|(_, w)| (*w - 1.0).abs() < 1e-6),
            "At least one interest should have normalized weight of 1.0"
        );
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum ModelTier {
    Verified,
    Experimental,
    Blocked,
}

#[allow(dead_code)] // REMOVE BY 2026-07-01
pub(crate) struct ModelEntry {
    pub family: &'static str,
    pub tier: ModelTier,
    pub min_ram_gb: f64,
    pub license: &'static str,
    pub notes: &'static str,
}

static ALLOWLIST: &[ModelEntry] = &[
    // ── Verified ───────────────────────────────────────────────────────
    ModelEntry {
        family: "qwen3:14b",
        tier: ModelTier::Verified,
        min_ram_gb: 11.0,
        license: "Apache-2.0",
        notes: "Primary — 5.4% hallucination, best structured output",
    },
    ModelEntry {
        family: "gemma3:12b",
        tier: ModelTier::Verified,
        min_ram_gb: 9.0,
        license: "Apache-2.0",
        notes: "Google QAT preserves quality at Q4",
    },
    ModelEntry {
        family: "qwen3:8b",
        tier: ModelTier::Verified,
        min_ram_gb: 7.0,
        license: "Apache-2.0",
        notes: "Minimum verified for synthesis",
    },
    // ── Experimental ───────────────────────────────────────────────────
    ModelEntry {
        family: "gemma3:4b",
        tier: ModelTier::Experimental,
        min_ram_gb: 4.0,
        license: "Apache-2.0",
        notes: "Anomalous IFEval 90.2, needs grammar constraints",
    },
    ModelEntry {
        family: "deepseek-r1",
        tier: ModelTier::Experimental,
        min_ram_gb: 5.0,
        license: "MIT",
        notes: "Reasoning model, may overthink structured tasks",
    },
    ModelEntry {
        family: "phi4",
        tier: ModelTier::Experimental,
        min_ram_gb: 5.0,
        license: "MIT",
        notes: "Microsoft, decent instruction following",
    },
    ModelEntry {
        family: "llama3.1:8b",
        tier: ModelTier::Experimental,
        min_ram_gb: 6.0,
        license: "Llama 3.1",
        notes: "Meta, was previous recommendation",
    },
    ModelEntry {
        family: "mistral",
        tier: ModelTier::Experimental,
        min_ram_gb: 6.0,
        license: "Apache-2.0",
        notes: "Older 7B, superseded by qwen3/gemma3",
    },
    // ── Blocked ────────────────────────────────────────────────────────
    ModelEntry {
        family: "llama3.2",
        tier: ModelTier::Blocked,
        min_ram_gb: 2.5,
        license: "Llama 3.2",
        notes: "3B default, produces hallucinated narratives",
    },
    ModelEntry {
        family: "phi3:mini",
        tier: ModelTier::Blocked,
        min_ram_gb: 2.5,
        license: "MIT",
        notes: "Too small for 20+ quality rules",
    },
    ModelEntry {
        family: "tinyllama",
        tier: ModelTier::Blocked,
        min_ram_gb: 1.0,
        license: "Apache-2.0",
        notes: "1.1B, immediately fails",
    },
];

/// Normalize a model tag for family matching: lowercase, strip `:latest` suffix.
fn normalize_tag(tag: &str) -> String {
    let lower = tag.to_lowercase();
    lower.strip_suffix(":latest").unwrap_or(&lower).to_string()
}

/// Find the best-matching allowlist entry for a tag. Longest family prefix wins
/// so that `qwen3:14b-q4_K_M` matches `qwen3:14b` over a hypothetical `qwen3`.
fn find_entry(model_tag: &str) -> Option<&'static ModelEntry> {
    let norm = normalize_tag(model_tag);
    ALLOWLIST
        .iter()
        .filter(|e| norm.starts_with(e.family))
        .max_by_key(|e| e.family.len())
}

/// Classify a model tag into a tier.
///
/// Unknown models default to `Experimental` (benefit of the doubt for
/// user-chosen models).
pub(crate) fn classify_model(model_tag: &str) -> ModelTier {
    find_entry(model_tag).map_or(ModelTier::Experimental, |e| e.tier)
}

/// Get the full allowlist entry for a model, if one exists.
#[allow(dead_code)] // REMOVE BY 2026-07-01
pub(crate) fn get_model_entry(model_tag: &str) -> Option<&'static ModelEntry> {
    find_entry(model_tag)
}

/// Recommend models that fit within `available_ram_gb`, ordered by preference
/// (Verified first, then Experimental; Blocked models are excluded).
pub(crate) fn recommend_models(available_ram_gb: f64) -> Vec<&'static ModelEntry> {
    let mut models: Vec<&ModelEntry> = ALLOWLIST
        .iter()
        .filter(|e| e.tier != ModelTier::Blocked && e.min_ram_gb <= available_ram_gb)
        .collect();

    models.sort_by(|a, b| {
        // Verified before Experimental
        let tier_ord = |t: ModelTier| match t {
            ModelTier::Verified => 0,
            ModelTier::Experimental => 1,
            ModelTier::Blocked => 2,
        };
        tier_ord(a.tier)
            .cmp(&tier_ord(b.tier))
            // Within the same tier, prefer larger models (more RAM = more capable)
            .then(
                b.min_ram_gb
                    .partial_cmp(&a.min_ram_gb)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    models
}

/// Get all models of a given tier.
#[allow(dead_code)] // REMOVE BY 2026-07-01
pub(crate) fn models_by_tier(tier: ModelTier) -> Vec<&'static ModelEntry> {
    ALLOWLIST.iter().filter(|e| e.tier == tier).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify_model ─────────────────────────────────────────────────

    #[test]
    fn verified_models_classified_correctly() {
        assert_eq!(classify_model("qwen3:14b"), ModelTier::Verified);
        assert_eq!(classify_model("gemma3:12b"), ModelTier::Verified);
        assert_eq!(classify_model("qwen3:8b"), ModelTier::Verified);
    }

    #[test]
    fn blocked_models_classified_correctly() {
        assert_eq!(classify_model("llama3.2"), ModelTier::Blocked);
        assert_eq!(classify_model("phi3:mini"), ModelTier::Blocked);
        assert_eq!(classify_model("tinyllama"), ModelTier::Blocked);
    }

    #[test]
    fn experimental_models_classified_correctly() {
        assert_eq!(classify_model("deepseek-r1"), ModelTier::Experimental);
        assert_eq!(classify_model("phi4"), ModelTier::Experimental);
        assert_eq!(classify_model("mistral"), ModelTier::Experimental);
    }

    #[test]
    fn unknown_model_defaults_to_experimental() {
        assert_eq!(classify_model("solar:10.7b"), ModelTier::Experimental);
        assert_eq!(classify_model("command-r:35b"), ModelTier::Experimental);
    }

    // ── family matching edge cases ─────────────────────────────────────

    #[test]
    fn latest_suffix_stripped() {
        assert_eq!(classify_model("llama3.2:latest"), ModelTier::Blocked);
        assert_eq!(classify_model("qwen3:14b:latest"), ModelTier::Verified);
    }

    #[test]
    fn quantization_suffix_matches() {
        assert_eq!(classify_model("qwen3:14b-q4_K_M"), ModelTier::Verified);
        assert_eq!(classify_model("gemma3:12b-q5_K_S"), ModelTier::Verified);
    }

    #[test]
    fn case_insensitive_matching() {
        assert_eq!(classify_model("Qwen3:14B"), ModelTier::Verified);
        assert_eq!(classify_model("TINYLLAMA"), ModelTier::Blocked);
    }

    // ── get_model_entry ────────────────────────────────────────────────

    #[test]
    fn entry_found_for_known_model() {
        let entry = get_model_entry("qwen3:14b").unwrap();
        assert_eq!(entry.family, "qwen3:14b");
        assert_eq!(entry.tier, ModelTier::Verified);
        assert!((entry.min_ram_gb - 11.0).abs() < f64::EPSILON);
        assert_eq!(entry.license, "Apache-2.0");
    }

    #[test]
    fn entry_none_for_unknown_model() {
        assert!(get_model_entry("solar:10.7b").is_none());
    }

    // ── recommend_models ───────────────────────────────────────────────

    #[test]
    fn recommend_excludes_models_above_ram() {
        let recs = recommend_models(8.0);
        for entry in &recs {
            assert!(entry.min_ram_gb <= 8.0);
        }
    }

    #[test]
    fn recommend_excludes_blocked() {
        let recs = recommend_models(64.0);
        for entry in &recs {
            assert_ne!(entry.tier, ModelTier::Blocked);
        }
    }

    #[test]
    fn recommend_verified_before_experimental() {
        let recs = recommend_models(64.0);
        let first_experimental = recs.iter().position(|e| e.tier == ModelTier::Experimental);
        let last_verified = recs.iter().rposition(|e| e.tier == ModelTier::Verified);
        if let (Some(exp), Some(ver)) = (first_experimental, last_verified) {
            assert!(ver < exp);
        }
    }

    #[test]
    fn recommend_returns_empty_for_tiny_ram() {
        let recs = recommend_models(0.5);
        assert!(recs.is_empty());
    }

    // ── models_by_tier ─────────────────────────────────────────────────

    #[test]
    fn models_by_tier_counts() {
        assert_eq!(models_by_tier(ModelTier::Verified).len(), 3);
        assert_eq!(models_by_tier(ModelTier::Experimental).len(), 5);
        assert_eq!(models_by_tier(ModelTier::Blocked).len(), 3);
    }
}

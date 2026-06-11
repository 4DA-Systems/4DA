// SPDX-License-Identifier: FSL-1.1-Apache-2.0
use crate::context_engine;
use crate::embedding_calibration;
use crate::scoring_config;
use fourda_macros::score_component;

/// Calibrate a raw similarity score (typically compressed in [0.3-0.6]) into
/// a spread distribution using a sigmoid stretch. Uses adaptive parameters
/// from `embedding_calibration` (auto-computed from observed distribution,
/// known-model lookup, or DSL defaults). This fixes the "everything scores
/// 45-50%" problem regardless of which embedding model the user runs.
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn calibrate_score(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    if raw >= 1.0 {
        return 1.0;
    }
    let center = embedding_calibration::get_sigmoid_center();
    let scale = embedding_calibration::get_sigmoid_scale();
    1.0 / (1.0 + ((center - raw) * scale).exp())
}

/// Compute interest score by comparing item embedding against interest embeddings
#[score_component(output_range = "0.0..=1.0")]
pub(crate) fn compute_interest_score(
    item_embedding: &[f32],
    interests: &[context_engine::Interest],
) -> f32 {
    compute_interest_score_for(item_embedding, interests, None)
}

/// Profile-aware variant of [`compute_interest_score`]: broad terms that are
/// the user's own detected domain (e.g. "ml" for an ML engineer) keep full
/// specificity weight instead of the broad-term discount.
pub(crate) fn compute_interest_score_for(
    item_embedding: &[f32],
    interests: &[context_engine::Interest],
    profile: Option<&SpecificityProfile>,
) -> f32 {
    if interests.is_empty() {
        return 0.0;
    }

    // Pre-compute item embedding norm once (hot loop optimization)
    let item_norm = crate::vector_norm(item_embedding);
    if item_norm < f32::EPSILON {
        return 0.0; // Zero-norm embedding can't produce meaningful similarity
    }
    let mut max_score: f32 = 0.0;

    for interest in interests {
        if let Some(ref interest_embedding) = interest.embedding {
            let similarity =
                crate::cosine_similarity_with_norm(item_embedding, item_norm, interest_embedding);
            let specificity = embedding_specificity_weight_for(&interest.topic, profile);
            let weighted = similarity * interest.weight * specificity;
            max_score = max_score.max(weighted);
        }
    }

    max_score
}

/// Known broad/generic interest terms that match too many items.
/// These get reduced keyword weight to prevent flooding.
pub(crate) const BROAD_INTEREST_TERMS: &[&str] = &[
    "open source",
    "ai",
    "ml",
    "cloud",
    "web",
    "programming",
    "software",
    "technology",
    "development",
    "coding",
    "data",
    "security",
    "devops",
    "backend",
    "frontend",
    "fullstack",
    "machine learning",
    "artificial intelligence",
    "deep learning",
    "tech",
    "engineering",
    "developer",
    "startup",
    "saas",
    "testing",
    "framework",
    "performance",
    "api",
    "database",
    "automation",
    "monitoring",
    "infrastructure",
    "containers",
    "microservices",
    "serverless",
    "tutorial",
    "best practices",
    "tooling",
];

/// Specificity weight for embedding-based interest matching (no profile).
/// Test-only convenience: production paths use the `_for` variant.
#[cfg(test)]
pub(crate) fn embedding_specificity_weight(interest_topic: &str) -> f32 {
    embedding_specificity_weight_for(interest_topic, None)
}

/// Profile-aware specificity weight: a broad term that IS the user's detected
/// primary domain (see [`SpecificityProfile::exempts_broad`]) gets full weight.
pub(crate) fn embedding_specificity_weight_for(
    interest_topic: &str,
    profile: Option<&SpecificityProfile>,
) -> f32 {
    let topic_lower = interest_topic.to_lowercase();
    let is_broad = BROAD_INTEREST_TERMS
        .iter()
        .any(|b| topic_lower == *b || topic_lower.contains(b));
    if is_broad && !profile.is_some_and(|p| p.exempts_broad(&topic_lower)) {
        scoring_config::SPECIFICITY_EMBEDDING_BROAD
    } else {
        1.0
    }
}

// ============================================================================
// Profile-aware broad-term exemption
// ============================================================================

/// Broad terms exempted per user role. "security" IS the primary domain for a
/// security engineer; penalizing it as too-broad blinds that persona to its
/// own field. Roles come in two naming families: inferred role slugs from
/// `role_inference.rs` ("security_engineer") and explicit onboarding values
/// ("security"). Both are honored.
const ROLE_BROAD_EXEMPTIONS: &[(&str, &[&str])] = &[
    ("security_engineer", &["security"]),
    ("security", &["security"]),
    (
        "ml_engineer",
        &[
            "ai",
            "ml",
            "machine learning",
            "artificial intelligence",
            "deep learning",
        ],
    ),
    ("data_engineer", &["data"]),
    ("data", &["data"]),
    (
        "devops_engineer",
        &[
            "devops",
            "infrastructure",
            "monitoring",
            "containers",
            "automation",
        ],
    ),
    (
        "devops",
        &[
            "devops",
            "infrastructure",
            "monitoring",
            "containers",
            "automation",
        ],
    ),
];

/// Minimal view of the user's detected identity used to decide whether a
/// broad interest term is actually that user's primary domain. Borrowed from
/// `ScoringContext` at the call site - no new context fields, no allocation.
pub(crate) struct SpecificityProfile<'a> {
    /// Resolved user role (inferred or explicit), lowercase slug.
    pub user_role: Option<&'a str>,
    /// Lowercase primary stack from the domain profile.
    pub primary_stack: &'a std::collections::HashSet<String>,
    /// Lowercase combined tech set from the domain profile.
    pub all_tech: &'a std::collections::HashSet<String>,
}

impl<'a> SpecificityProfile<'a> {
    pub(crate) fn from_ctx(ctx: &'a super::ScoringContext) -> Self {
        Self {
            user_role: ctx.user_role.as_deref(),
            primary_stack: &ctx.domain_profile.primary_stack,
            all_tech: &ctx.domain_profile.all_tech,
        }
    }

    /// True when `topic_lower` (an exact lowercase interest topic) is part of
    /// this user's detected domain and must NOT receive the broad-term
    /// discount. Exact matching only - obvious variants are covered by the
    /// per-role exemption lists.
    pub(crate) fn exempts_broad(&self, topic_lower: &str) -> bool {
        // Literal: the term itself appears in the user's detected stack.
        if self.primary_stack.contains(topic_lower) || self.all_tech.contains(topic_lower) {
            return true;
        }
        // Role-derived: the term is the primary domain of the user's role.
        if let Some(role) = self.user_role {
            return ROLE_BROAD_EXEMPTIONS
                .iter()
                .any(|(r, terms)| *r == role && terms.contains(&topic_lower));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_specificity_broad_attenuated() {
        assert_eq!(embedding_specificity_weight("Open Source"), 0.40);
        assert_eq!(embedding_specificity_weight("AI"), 0.40);
        assert_eq!(embedding_specificity_weight("machine learning"), 0.40);
    }

    #[test]
    fn test_embedding_specificity_specific_full() {
        assert_eq!(embedding_specificity_weight("Tauri"), 1.0);
        assert_eq!(embedding_specificity_weight("rust"), 1.0);
        assert_eq!(embedding_specificity_weight("sqlite-vss"), 1.0);
    }

    #[test]
    fn test_broad_terms_include_expanded_set() {
        for term in &[
            "testing",
            "framework",
            "performance",
            "api",
            "database",
            "automation",
            "monitoring",
            "infrastructure",
            "containers",
            "microservices",
            "serverless",
            "tutorial",
            "best practices",
            "tooling",
        ] {
            assert_eq!(
                embedding_specificity_weight(term),
                0.40,
                "'{term}' should be classified as broad"
            );
        }
    }

    // ====================================================================
    // calibrate_score tests
    // ====================================================================

    #[test]
    fn test_calibrate_score_zero() {
        assert_eq!(calibrate_score(0.0), 0.0);
    }

    #[test]
    fn test_calibrate_score_one() {
        assert_eq!(calibrate_score(1.0), 1.0);
    }

    #[test]
    fn test_calibrate_score_negative() {
        assert_eq!(calibrate_score(-0.5), 0.0);
    }

    #[test]
    fn test_calibrate_score_above_one() {
        assert_eq!(calibrate_score(1.5), 1.0);
    }

    #[test]
    fn test_calibrate_score_midpoint() {
        // At the sigmoid center, output should be close to 0.5
        let center = crate::embedding_calibration::get_sigmoid_center();
        let cal = calibrate_score(center);
        assert!(
            (cal - 0.5).abs() < 0.05,
            "At sigmoid center ({}), calibrated should be ~0.5, got {}",
            center,
            cal
        );
    }

    #[test]
    fn test_calibrate_score_monotonic() {
        // Calibration should be monotonically increasing
        let values: Vec<f32> = (0..=10).map(|i| i as f32 / 10.0).collect();
        let calibrated: Vec<f32> = values.iter().map(|&v| calibrate_score(v)).collect();
        for i in 0..calibrated.len() - 1 {
            assert!(
                calibrated[i] <= calibrated[i + 1],
                "calibrate_score should be monotonic: {} > {} at inputs ({}, {})",
                calibrated[i],
                calibrated[i + 1],
                values[i],
                values[i + 1]
            );
        }
    }

    #[test]
    fn test_calibrate_score_spreads_midrange() {
        // The typical [0.40-0.56] band should spread to a wider range
        let low_mid = calibrate_score(0.40);
        let high_mid = calibrate_score(0.56);
        let spread = high_mid - low_mid;
        assert!(
            spread > 0.3,
            "Midrange [0.40-0.56] should spread to >0.3 range, got {}",
            spread
        );
    }

    // ====================================================================
    // embedding_specificity_weight edge cases
    // ====================================================================

    #[test]
    fn test_embedding_specificity_case_insensitive() {
        assert_eq!(embedding_specificity_weight("OPEN SOURCE"), 0.40);
        assert_eq!(embedding_specificity_weight("Machine Learning"), 0.40);
    }

    #[test]
    fn test_embedding_specificity_contains_broad() {
        // "artificial intelligence" contains "ai"
        assert_eq!(
            embedding_specificity_weight("artificial intelligence"),
            0.40
        );
    }

    #[test]
    fn test_embedding_specificity_empty_string() {
        // Empty string doesn't match any broad term
        assert_eq!(embedding_specificity_weight(""), 1.0);
    }

    // ====================================================================
    // BROAD_INTEREST_TERMS coverage
    // ====================================================================

    // ====================================================================
    // Profile-aware broad-term exemption
    // ====================================================================

    use std::collections::HashSet;

    fn tech_set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_security_stays_broad_for_react_dev() {
        let stack = tech_set(&["react", "typescript"]);
        let all = stack.clone();
        let profile = SpecificityProfile {
            user_role: Some("frontend_developer"),
            primary_stack: &stack,
            all_tech: &all,
        };
        assert_eq!(
            embedding_specificity_weight_for("security", Some(&profile)),
            0.40,
            "'security' must keep the broad penalty for a frontend dev"
        );
        assert_eq!(
            embedding_specificity_weight_for("ml", Some(&profile)),
            0.40,
            "'ml' must keep the broad penalty for a frontend dev"
        );
    }

    #[test]
    fn test_security_not_broad_for_security_stack() {
        // The full chain: a stack of security tooling infers security_engineer,
        // and that role exempts "security" from the broad-term discount.
        let tooling: Vec<String> = ["nmap", "metasploit", "wireshark"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let role = crate::scoring::role_inference::infer_role(&tooling);
        assert_eq!(role.as_deref(), Some("security_engineer"));

        let stack = tech_set(&["nmap", "metasploit", "wireshark"]);
        let all = stack.clone();
        let profile = SpecificityProfile {
            user_role: role.as_deref(),
            primary_stack: &stack,
            all_tech: &all,
        };
        assert_eq!(
            embedding_specificity_weight_for("security", Some(&profile)),
            1.0,
            "'security' is the primary domain for a security engineer"
        );
        // Other broad terms stay penalized for this persona.
        assert_eq!(
            embedding_specificity_weight_for("web", Some(&profile)),
            0.40
        );
    }

    #[test]
    fn test_ml_not_broad_for_ml_stack() {
        let tooling: Vec<String> = ["pytorch", "transformers"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let role = crate::scoring::role_inference::infer_role(&tooling);
        assert_eq!(role.as_deref(), Some("ml_engineer"));

        let stack = tech_set(&["pytorch", "transformers"]);
        let all = stack.clone();
        let profile = SpecificityProfile {
            user_role: role.as_deref(),
            primary_stack: &stack,
            all_tech: &all,
        };
        for term in ["ml", "ai", "machine learning", "deep learning"] {
            assert_eq!(
                embedding_specificity_weight_for(term, Some(&profile)),
                1.0,
                "'{term}' is the primary domain for an ml engineer"
            );
        }
    }

    #[test]
    fn test_literal_stack_term_exempted() {
        // A broad term that literally appears in the detected stack is the
        // user's own domain regardless of role.
        let stack = tech_set(&["devops", "terraform"]);
        let all = stack.clone();
        let profile = SpecificityProfile {
            user_role: None,
            primary_stack: &stack,
            all_tech: &all,
        };
        assert_eq!(
            embedding_specificity_weight_for("devops", Some(&profile)),
            1.0
        );
    }

    #[test]
    fn test_no_profile_keeps_existing_behavior() {
        assert_eq!(embedding_specificity_weight_for("security", None), 0.40);
        assert_eq!(embedding_specificity_weight_for("tauri", None), 1.0);
    }

    #[test]
    fn test_broad_interest_terms_complete() {
        // Verify key terms are in the list
        let key_terms = [
            "open source",
            "ai",
            "ml",
            "cloud",
            "web",
            "programming",
            "software",
            "technology",
            "development",
            "security",
        ];
        for term in &key_terms {
            assert!(
                BROAD_INTEREST_TERMS.contains(term),
                "'{}' should be in BROAD_INTEREST_TERMS",
                term
            );
        }
    }
}

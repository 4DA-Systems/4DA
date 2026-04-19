// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Auto-detect stack profiles from ACE context.
//!
//! Matches ACE-detected technologies against each profile's detection markers
//! and core_tech to suggest relevant profiles during onboarding.

use super::profiles::ALL_PROFILES;
use super::scoring::text_contains_term;
use crate::scoring::ACEContext;

/// A detected stack profile with confidence.
#[derive(Debug, Clone)]
pub struct StackDetection {
    pub profile_id: String,
    pub profile_name: String,
    pub confidence: f32,
    pub matched_tech: Vec<String>,
}

/// Detect which stack profiles match the user's ACE context.
///
/// Matches detected_tech and dependency_names against each profile's
/// core_tech and detection_markers. Requires `detection_threshold`
/// matches for a positive detection.
pub(crate) fn detect_matching_profiles(ace_ctx: &ACEContext) -> Vec<StackDetection> {
    let mut detections = Vec::new();

    for &profile in &ALL_PROFILES {
        let mut matched: Vec<String> = Vec::new();

        // Check core_tech against detected_tech
        for &tech in profile.core_tech {
            if ace_ctx
                .detected_tech
                .iter()
                .any(|dt| dt.eq_ignore_ascii_case(tech))
            {
                matched.push(tech.to_string());
            }
        }

        // Check detection_markers against detected_tech + dependency_names
        for &marker in profile.detection_markers {
            let marker_lower = marker.to_lowercase();
            let already_matched = matched.iter().any(|m| m.eq_ignore_ascii_case(marker));
            if already_matched {
                continue;
            }

            let in_detected = ace_ctx.detected_tech.iter().any(|dt| {
                let dt_lower = dt.to_lowercase();
                dt_lower == marker_lower || text_contains_term(&dt_lower, &marker_lower)
            });
            let in_deps = ace_ctx.dependency_names.contains(&marker_lower);

            if in_detected || in_deps {
                matched.push(marker.to_string());
            }
        }

        // Check companions against dependency_names
        for &companion in profile.companions {
            let companion_lower = companion.to_lowercase();
            let already_matched = matched.iter().any(|m| m.eq_ignore_ascii_case(companion));
            if already_matched {
                continue;
            }
            if ace_ctx.dependency_names.contains(&companion_lower) {
                matched.push(companion.to_string());
            }
        }

        if matched.len() >= profile.detection_threshold {
            // Confidence scales with match count (more matches = more confident)
            let max_possible = profile.core_tech.len() + profile.detection_markers.len();
            let confidence = (matched.len() as f32 / max_possible as f32).min(1.0);

            detections.push(StackDetection {
                profile_id: profile.id.to_string(),
                profile_name: profile.name.to_string(),
                confidence,
                matched_tech: matched,
            });
        }
    }

    // Sort by confidence (highest first)
    detections.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    detections
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn make_ace_ctx(tech: &[&str], deps: &[&str]) -> ACEContext {
        ACEContext {
            detected_tech: tech.iter().map(|s| s.to_string()).collect(),
            dependency_names: deps.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
            active_topics: Vec::new(),
            topic_confidence: HashMap::new(),
            anti_topics: Vec::new(),
            anti_topic_confidence: HashMap::new(),
            topic_affinities: HashMap::new(),
            dependency_info: HashMap::new(),
            peak_hours: Vec::new(),
            tech_weights: HashMap::new(),
            negative_stack: Default::default(),
        }
    }

    #[test]
    fn test_detect_rust_profile() {
        let ctx = make_ace_ctx(&["rust", "cargo"], &["tokio", "serde"]);
        let detections = detect_matching_profiles(&ctx);
        assert!(
            detections.iter().any(|d| d.profile_id == "rust_systems"),
            "Should detect rust_systems, got: {:?}",
            detections.iter().map(|d| &d.profile_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_detect_nextjs_profile() {
        let ctx = make_ace_ctx(&["typescript", "react"], &["nextjs", "vercel"]);
        let detections = detect_matching_profiles(&ctx);
        assert!(
            detections
                .iter()
                .any(|d| d.profile_id == "nextjs_fullstack"),
            "Should detect nextjs_fullstack"
        );
    }

    #[test]
    fn test_no_detection_without_threshold() {
        // Only 1 match for profiles needing 2
        let ctx = make_ace_ctx(&["python"], &[]);
        let detections = detect_matching_profiles(&ctx);
        // python_ml needs at least 2 markers
        let ml_detected = detections.iter().any(|d| d.profile_id == "python_ml");
        // It might detect if "python" matches core_tech + detection_markers
        // but with only 1 unique match it shouldn't pass threshold
        if ml_detected {
            let d = detections
                .iter()
                .find(|d| d.profile_id == "python_ml")
                .unwrap();
            assert!(d.matched_tech.len() >= 2);
        }
    }

    #[test]
    fn test_empty_context_no_detections() {
        let ctx = make_ace_ctx(&[], &[]);
        let detections = detect_matching_profiles(&ctx);
        assert!(detections.is_empty());
    }

    #[test]
    fn test_multi_profile_detection() {
        // User with both React and TypeScript — could match nextjs + react_native
        let ctx = make_ace_ctx(
            &["typescript", "react", "nextjs"],
            &["react-native", "expo"],
        );
        let detections = detect_matching_profiles(&ctx);
        assert!(detections.len() >= 2, "Should detect multiple profiles");
    }

    #[test]
    fn test_confidence_scaling() {
        let ctx = make_ace_ctx(&["rust", "cargo", "tokio", "serde"], &["axum", "tracing"]);
        let detections = detect_matching_profiles(&ctx);
        let rust = detections.iter().find(|d| d.profile_id == "rust_systems");
        assert!(rust.is_some());
        let rust = rust.unwrap();
        assert!(
            rust.confidence > 0.3,
            "High match count should give decent confidence, got {}",
            rust.confidence
        );
    }
}

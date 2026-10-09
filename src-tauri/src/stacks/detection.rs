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
///
/// A detection also needs EVIDENCE OF THE STACK ITSELF: at least one of the
/// profile's `detection_markers` (its fingerprint — `next.config`,
/// `react-native`, `Cargo.toml`, ...). Shared ecosystem tech is not enough:
/// a Tauri app's React + TypeScript + zustand matched "Next.js Fullstack"
/// (27%) and "React Native" (33%) and onboarding auto-ticked both
/// (fresh-profile E2E, 2026-10-09). Companions still show in `matched_tech`
/// but neither satisfy the anchor nor raise the confidence: they are what a
/// stack's users ALSO use, so on their own they say nothing about which stack.
pub(crate) fn detect_matching_profiles(ace_ctx: &ACEContext) -> Vec<StackDetection> {
    let mut detections = Vec::new();

    for &profile in &ALL_PROFILES {
        let mut matched: Vec<String> = Vec::new();
        let mut anchored = false;

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
            let in_detected = ace_ctx.detected_tech.iter().any(|dt| {
                let dt_lower = dt.to_lowercase();
                dt_lower == marker_lower || text_contains_term(&dt_lower, &marker_lower)
            });
            let in_deps = ace_ctx.dependency_names.contains(&marker_lower);
            if !(in_detected || in_deps) {
                continue;
            }
            anchored = true;
            if !matched.iter().any(|m| m.eq_ignore_ascii_case(marker)) {
                matched.push(marker.to_string());
            }
        }
        // Core tech + markers: the evidence the confidence is computed from.
        let defining_matches = matched.len();

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

        if anchored && matched.len() >= profile.detection_threshold {
            // Confidence scales with the defining matches (core tech +
            // markers) over everything the profile could match on.
            let max_possible = profile.core_tech.len() + profile.detection_markers.len();
            let confidence = (defining_matches as f32 / max_possible as f32).min(1.0);

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
            dependency_info: HashMap::new(),
            peak_hours: Vec::new(),
            tech_weights: HashMap::new(),
            negative_stack: Default::default(),
            tech_projects: Default::default(),
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

    /// Fresh-profile E2E 2026-10-09: a Tauri/Rust repo with a React +
    /// TypeScript frontend (the D:\4DA shape — detected_tech as ACE records
    /// it for this repo) auto-ticked "Next.js Fullstack" and "React Native".
    /// Neither stack's fingerprint is present; Rust's is.
    #[test]
    fn tauri_react_repo_detects_rust_not_nextjs_or_react_native() {
        let ctx = make_ace_ctx(
            &[
                "rust",
                "javascript",
                "typescript",
                "react",
                "axum",
                "tauri",
                "tokio",
                "@tauri-apps/api",
                "react-dom",
                "serde",
                "reqwest",
            ],
            &[
                "tokio",
                "serde",
                "tauri",
                "rusqlite",
                "axum",
                "tracing",
                "react",
                "react-dom",
                "zustand",
                "zod",
                "tailwindcss",
                "vitest",
                "i18next",
            ],
        );
        let detections = detect_matching_profiles(&ctx);
        let ids: Vec<&str> = detections.iter().map(|d| d.profile_id.as_str()).collect();
        assert!(ids.contains(&"rust_systems"), "{ids:?}");
        assert!(!ids.contains(&"nextjs_fullstack"), "{ids:?}");
        assert!(!ids.contains(&"react_native"), "{ids:?}");
        let rust = &detections[0];
        assert_eq!(rust.profile_id, "rust_systems", "Rust ranks first");
        assert!(rust.confidence >= 0.25, "{}", rust.confidence);
    }

    #[test]
    fn companions_alone_never_detect_a_stack() {
        // React + zustand + TypeScript: all React Native companions/core, no
        // react-native / expo fingerprint.
        let ctx = make_ace_ctx(&["typescript", "react"], &["zustand", "react"]);
        let ids: Vec<String> = detect_matching_profiles(&ctx)
            .into_iter()
            .map(|d| d.profile_id)
            .collect();
        assert!(!ids.iter().any(|i| i == "react_native"), "{ids:?}");
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

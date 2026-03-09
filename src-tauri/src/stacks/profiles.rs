//! Stack profile definitions — all 11 built-in technology profiles.
//!
//! Each profile encodes curated domain knowledge about a technology ecosystem:
//! pain points, ecosystem shifts, keyword boosts, source preferences, and
//! auto-detection markers.
//!
//! Profile data is split into sibling modules to stay under file-size limits.

use super::StackProfile;

// --- Sibling modules ---

#[path = "profile_data_a.rs"]
mod profile_data_a;
pub use profile_data_a::*;

#[path = "profile_data_b.rs"]
mod profile_data_b;
pub use profile_data_b::*;

// ============================================================================
// Profile Registry
// ============================================================================

/// All available stack profiles, in display order.
pub static ALL_PROFILES: [&StackProfile; 11] = [
    &NEXTJS_FULLSTACK,
    &RUST_SYSTEMS,
    &PYTHON_ML,
    &GO_BACKEND,
    &REACT_NATIVE,
    &LARAVEL,
    &DJANGO,
    &VUE_FRONTEND,
    &DEVOPS_SRE,
    &HASKELL_FP,
    &BOOTSTRAP_WEBDEV,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_profiles_have_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for profile in &ALL_PROFILES {
            assert!(
                ids.insert(profile.id),
                "Duplicate profile ID: {}",
                profile.id
            );
        }
    }

    #[test]
    fn test_all_profiles_have_pain_points() {
        for profile in &ALL_PROFILES {
            assert!(
                !profile.pain_points.is_empty(),
                "{} has no pain points",
                profile.id
            );
            for pp in profile.pain_points {
                assert!(
                    pp.keywords.len() >= 2,
                    "{}: pain point '{}' needs 2+ keywords",
                    profile.id,
                    pp.description
                );
                assert!(
                    pp.severity >= 0.05 && pp.severity <= 0.20,
                    "{}: pain point severity out of range: {}",
                    profile.id,
                    pp.severity
                );
            }
        }
    }

    #[test]
    fn test_all_profiles_have_ecosystem_shifts() {
        for profile in &ALL_PROFILES {
            assert!(
                !profile.ecosystem_shifts.is_empty(),
                "{} has no ecosystem shifts",
                profile.id
            );
            for es in profile.ecosystem_shifts {
                assert!(
                    es.boost >= 1.0 && es.boost <= 1.25,
                    "{}: ecosystem shift boost out of range: {}",
                    profile.id,
                    es.boost
                );
            }
        }
    }

    #[test]
    fn test_no_core_tech_in_competing() {
        for profile in &ALL_PROFILES {
            for &core in profile.core_tech {
                assert!(
                    !profile.competing.contains(&core),
                    "{}: '{}' is both core_tech and competing",
                    profile.id,
                    core
                );
            }
        }
    }

    #[test]
    fn test_detection_threshold_reasonable() {
        for profile in &ALL_PROFILES {
            assert!(
                profile.detection_threshold >= 1 && profile.detection_threshold <= 3,
                "{}: detection_threshold {} seems wrong",
                profile.id,
                profile.detection_threshold
            );
            assert!(
                profile.detection_markers.len() >= profile.detection_threshold,
                "{}: not enough detection markers ({}) for threshold ({})",
                profile.id,
                profile.detection_markers.len(),
                profile.detection_threshold
            );
        }
    }
}

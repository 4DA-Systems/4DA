//! Achievement definitions for the GAME Engine.
//!
//! Extracted from achievement_engine.rs to keep file sizes manageable.
//! Contains the AchievementTier enum and all 25 achievement definitions.

use serde::{Deserialize, Serialize};

/// Achievement tier — determines celebration intensity and badge color.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AchievementTier {
    Bronze,
    Silver,
    Gold,
}

impl AchievementTier {
    pub fn intensity(&self) -> f64 {
        match self {
            AchievementTier::Bronze => 0.5,
            AchievementTier::Silver => 0.75,
            AchievementTier::Gold => 1.0,
        }
    }
}

/// Achievement definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub counter_type: String,
    pub threshold: u64,
    pub tier: AchievementTier,
}

/// All 25 achievements
pub fn all_achievements() -> Vec<Achievement> {
    vec![
        // --- Scans ---
        Achievement {
            id: "first_scan".into(),
            name: "First Light".into(),
            description: "Run your first content scan".into(),
            icon: "telescope".into(),
            counter_type: "scans".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        Achievement {
            id: "ten_scans".into(),
            name: "Radar Operator".into(),
            description: "Run 10 content scans".into(),
            icon: "satellite".into(),
            counter_type: "scans".into(),
            threshold: 10,
            tier: AchievementTier::Silver,
        },
        Achievement {
            id: "fifty_scans".into(),
            name: "Signal Hunter".into(),
            description: "Run 50 content scans".into(),
            icon: "radar".into(),
            counter_type: "scans".into(),
            threshold: 50,
            tier: AchievementTier::Gold,
        },
        // --- Discoveries ---
        Achievement {
            id: "first_discovery".into(),
            name: "Eureka".into(),
            description: "Find your first relevant item".into(),
            icon: "lightbulb".into(),
            counter_type: "discoveries".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        Achievement {
            id: "ten_discoveries".into(),
            name: "Pattern Spotter".into(),
            description: "Find 10 relevant items".into(),
            icon: "eye".into(),
            counter_type: "discoveries".into(),
            threshold: 10,
            tier: AchievementTier::Silver,
        },
        Achievement {
            id: "hundred_discoveries".into(),
            name: "Intelligence Analyst".into(),
            description: "Find 100 relevant items".into(),
            icon: "brain".into(),
            counter_type: "discoveries".into(),
            threshold: 100,
            tier: AchievementTier::Gold,
        },
        // --- Saves ---
        Achievement {
            id: "first_save".into(),
            name: "Collector".into(),
            description: "Save your first item".into(),
            icon: "bookmark".into(),
            counter_type: "saves".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        // --- Briefings ---
        Achievement {
            id: "first_briefing".into(),
            name: "Briefed".into(),
            description: "Generate your first briefing".into(),
            icon: "newspaper".into(),
            counter_type: "briefings".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        // --- Sources ---
        Achievement {
            id: "three_sources".into(),
            name: "Multi-Source".into(),
            description: "Discover items from 3+ sources".into(),
            icon: "antenna".into(),
            counter_type: "sources".into(),
            threshold: 3,
            tier: AchievementTier::Silver,
        },
        Achievement {
            id: "five_sources".into(),
            name: "Intel Network".into(),
            description: "Discover items from 5+ sources".into(),
            icon: "globe".into(),
            counter_type: "sources".into(),
            threshold: 5,
            tier: AchievementTier::Silver,
        },
        // --- Context ---
        Achievement {
            id: "context_builder".into(),
            name: "Context Builder".into(),
            description: "Set up 3 context items (role, tech, interests)".into(),
            icon: "puzzle".into(),
            counter_type: "context".into(),
            threshold: 3,
            tier: AchievementTier::Silver,
        },
        // --- Streak ---
        Achievement {
            id: "streak_three".into(),
            name: "Consistent".into(),
            description: "Use 4DA 3 days in a row".into(),
            icon: "flame".into(),
            counter_type: "streak".into(),
            threshold: 3,
            tier: AchievementTier::Silver,
        },
        Achievement {
            id: "streak_seven".into(),
            name: "Dedicated".into(),
            description: "Use 4DA 7 days in a row".into(),
            icon: "fire".into(),
            counter_type: "streak".into(),
            threshold: 7,
            tier: AchievementTier::Silver,
        },
        // --- Decisions ---
        Achievement {
            id: "first_decision".into(),
            name: "Decision Made".into(),
            description: "Record your first decision".into(),
            icon: "gavel".into(),
            counter_type: "decisions".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        Achievement {
            id: "ten_decisions".into(),
            name: "Decisive".into(),
            description: "Record 10 decisions".into(),
            icon: "scales".into(),
            counter_type: "decisions".into(),
            threshold: 10,
            tier: AchievementTier::Silver,
        },
        Achievement {
            id: "fifty_decisions".into(),
            name: "Strategic Mind".into(),
            description: "Record 50 decisions".into(),
            icon: "chess".into(),
            counter_type: "decisions".into(),
            threshold: 50,
            tier: AchievementTier::Gold,
        },
        // --- Calibrations ---
        Achievement {
            id: "first_calibration".into(),
            name: "Calibrated".into(),
            description: "Complete your first calibration".into(),
            icon: "target".into(),
            counter_type: "calibrations".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        Achievement {
            id: "five_calibrations".into(),
            name: "Fine-Tuned".into(),
            description: "Complete 5 calibrations".into(),
            icon: "sliders".into(),
            counter_type: "calibrations".into(),
            threshold: 5,
            tier: AchievementTier::Silver,
        },
        // --- Channels ---
        Achievement {
            id: "first_channel".into(),
            name: "Tuned In".into(),
            description: "Add your first channel".into(),
            icon: "radio".into(),
            counter_type: "channels".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        Achievement {
            id: "five_channels".into(),
            name: "Broadband".into(),
            description: "Add 5 channels".into(),
            icon: "antenna_bars".into(),
            counter_type: "channels".into(),
            threshold: 5,
            tier: AchievementTier::Silver,
        },
        // --- Taste Tests ---
        Achievement {
            id: "first_taste_test".into(),
            name: "Taste Tester".into(),
            description: "Complete your first taste test".into(),
            icon: "flask".into(),
            counter_type: "taste_tests".into(),
            threshold: 1,
            tier: AchievementTier::Bronze,
        },
        Achievement {
            id: "five_taste_tests".into(),
            name: "Connoisseur".into(),
            description: "Complete 5 taste tests".into(),
            icon: "wine".into(),
            counter_type: "taste_tests".into(),
            threshold: 5,
            tier: AchievementTier::Gold,
        },
        // --- Profile Updates ---
        Achievement {
            id: "five_profile_updates".into(),
            name: "Identity Forged".into(),
            description: "Update your profile 5 times".into(),
            icon: "shield".into(),
            counter_type: "profile_updates".into(),
            threshold: 5,
            tier: AchievementTier::Silver,
        },
        // --- Extended Streak ---
        Achievement {
            id: "streak_fourteen".into(),
            name: "Relentless".into(),
            description: "Use 4DA 14 days in a row".into(),
            icon: "lightning".into(),
            counter_type: "streak".into(),
            threshold: 14,
            tier: AchievementTier::Gold,
        },
        Achievement {
            id: "streak_thirty".into(),
            name: "Sovereign".into(),
            description: "Use 4DA 30 days in a row".into(),
            icon: "crown".into(),
            counter_type: "streak".into(),
            threshold: 30,
            tier: AchievementTier::Gold,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_achievements_count() {
        let achievements = all_achievements();
        assert_eq!(
            achievements.len(),
            25,
            "Should have exactly 25 achievements"
        );
    }

    #[test]
    fn test_all_achievements_unique_ids() {
        let achievements = all_achievements();
        let mut ids: Vec<&str> = achievements.iter().map(|a| a.id.as_str()).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total, "All achievement IDs should be unique");
    }

    #[test]
    fn test_all_achievements_have_required_fields() {
        for a in all_achievements() {
            assert!(!a.id.is_empty(), "Achievement ID should not be empty");
            assert!(!a.name.is_empty(), "Achievement name should not be empty");
            assert!(
                !a.description.is_empty(),
                "Achievement description should not be empty"
            );
            assert!(!a.icon.is_empty(), "Achievement icon should not be empty");
            assert!(
                !a.counter_type.is_empty(),
                "Achievement counter_type should not be empty"
            );
            assert!(a.threshold > 0, "Achievement threshold should be positive");
        }
    }

    #[test]
    fn test_all_achievements_counter_types() {
        let achievements = all_achievements();
        let valid_types = [
            "scans",
            "discoveries",
            "saves",
            "briefings",
            "sources",
            "context",
            "streak",
            "decisions",
            "calibrations",
            "channels",
            "taste_tests",
            "profile_updates",
        ];
        for a in &achievements {
            assert!(
                valid_types.contains(&a.counter_type.as_str()),
                "Unknown counter type '{}' in achievement '{}'",
                a.counter_type,
                a.id
            );
        }
    }

    #[test]
    fn test_scan_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let scan_thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "scans")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(scan_thresholds, vec![1, 10, 50]);
    }

    #[test]
    fn test_discovery_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "discoveries")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(thresholds, vec![1, 10, 100]);
    }

    #[test]
    fn test_streak_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "streak")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(thresholds, vec![3, 7, 14, 30]);
    }

    #[test]
    fn test_source_achievements_ordered_thresholds() {
        let achievements = all_achievements();
        let thresholds: Vec<u64> = achievements
            .iter()
            .filter(|a| a.counter_type == "sources")
            .map(|a| a.threshold)
            .collect();
        assert_eq!(thresholds, vec![3, 5]);
    }

    #[test]
    fn test_first_scan_achievement() {
        let achievements = all_achievements();
        let first = achievements.iter().find(|a| a.id == "first_scan").unwrap();
        assert_eq!(first.threshold, 1);
        assert_eq!(first.counter_type, "scans");
        assert_eq!(first.name, "First Light");
        assert_eq!(first.tier, AchievementTier::Bronze);
    }

    #[test]
    fn test_context_builder_achievement() {
        let achievements = all_achievements();
        let ctx = achievements
            .iter()
            .find(|a| a.id == "context_builder")
            .unwrap();
        assert_eq!(ctx.threshold, 3);
        assert_eq!(ctx.counter_type, "context");
        assert_eq!(ctx.tier, AchievementTier::Silver);
    }

    #[test]
    fn test_achievement_tiers_assigned() {
        for a in all_achievements() {
            // Every achievement must have a valid tier — the type system enforces
            // this, but we verify the assignment is intentional by checking intensity
            let intensity = a.tier.intensity();
            assert!(
                intensity > 0.0 && intensity <= 1.0,
                "Achievement '{}' has invalid tier intensity: {}",
                a.id,
                intensity
            );
        }
    }

    #[test]
    fn test_celebration_intensity() {
        assert!(
            (AchievementTier::Bronze.intensity() - 0.5).abs() < f64::EPSILON,
            "Bronze intensity should be 0.5"
        );
        assert!(
            (AchievementTier::Silver.intensity() - 0.75).abs() < f64::EPSILON,
            "Silver intensity should be 0.75"
        );
        assert!(
            (AchievementTier::Gold.intensity() - 1.0).abs() < f64::EPSILON,
            "Gold intensity should be 1.0"
        );
    }
}

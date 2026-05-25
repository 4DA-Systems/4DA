// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior types — user interaction signals, topic affinities, anti-topics.

use serde::{Deserialize, Serialize};

// ============================================================================
// Behavior Types
// ============================================================================

/// Context for saves — different contexts produce different decay rates and strengths
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SaveContext {
    /// Useful right now — boost intent + decision window relevance
    UsefulNow,
    /// Long-term reference — slower affinity decay
    Reference,
    /// Worth sharing — high-quality content signal
    Share,
}

/// Classifies the pattern of a click interaction beyond raw dwell time.
///
/// A 30-second dwell means very different things: the user could be reading
/// carefully (Engaged), reading confused and re-reading paragraphs (Confused),
/// or left a tab open while getting coffee (Abandoned). Without this
/// classification, every long dwell reads as "engagement" — the single
/// biggest failure mode of naive implicit-feedback systems.
///
/// Inferred by the frontend from scroll-to-bottom ratio, scroll direction
/// changes, back-button trigger, and dwell distribution. Emitted on item
/// close. Backward-compatible: when the frontend can't classify, `pattern`
/// on `Click` is `None` and the legacy dwell-only weight applies.
///
/// See `docs/strategy/INTELLIGENCE-MESH.md` and the Phase 6 behavior-pattern
/// plan for the full rationale.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionPattern {
    /// Short dwell + back trigger + minimal scroll. The item wasn't what
    /// the user expected. Weak negative signal, NOT positive.
    Bounced,
    /// Medium dwell + partial scroll + no return. The user read enough to
    /// form an opinion and moved on — neutral-to-positive.
    Scanned,
    /// Reasonable dwell + scroll progress + no return pattern. The user
    /// actively read the item — strong positive.
    Engaged,
    /// Dwell + scroll to end + no return. Full read-through.
    /// Strongest positive short of an explicit save/share.
    Completed,
    /// Scrolled back and forth significantly. Usually means re-reading to
    /// understand — not pure positive. Flags the item for a necessity
    /// boost on related introductory content (resolves confusion).
    Reread,
    /// Very long dwell with no scroll activity. Tab left open during coffee,
    /// not engagement. Treated as neutral — we don't punish, but we don't
    /// reward either.
    Abandoned,
}

impl InteractionPattern {
    /// Multiplier applied on top of the base Click strength.
    /// Range: -0.8 (bounced = actively wrong) to 1.4 (completed).
    pub fn strength_multiplier(&self) -> f32 {
        match self {
            InteractionPattern::Bounced => -0.4,
            InteractionPattern::Scanned => 0.8,
            InteractionPattern::Engaged => 1.1,
            InteractionPattern::Completed => 1.4,
            InteractionPattern::Reread => 0.6,
            InteractionPattern::Abandoned => 0.0,
        }
    }

    /// True when the pattern suggests the item was above the user's level.
    /// Reread + short-Bounced fall here; Engaged/Completed do not. Used by
    /// the necessity scorer to boost foundational content on the same topic.
    // REMOVE BY 2026-08-01
    #[allow(dead_code)] // Consumed by necessity-scorer hook in follow-up commit.
    pub fn suggests_above_level(&self) -> bool {
        matches!(
            self,
            InteractionPattern::Reread | InteractionPattern::Bounced
        )
    }
}

/// Types of user behavior we track
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BehaviorAction {
    Click {
        dwell_time_seconds: u64,
        /// Inferred interaction pattern. `None` preserves legacy dwell-only
        /// scoring; `Some(_)` applies pattern-aware strength via
        /// `InteractionPattern::strength_multiplier`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pattern: Option<InteractionPattern>,
    },
    Save,
    Share,
    Dismiss,
    MarkIrrelevant,
    Scroll {
        visible_seconds: f32,
    },
    Ignore,
    /// User clicked an item in the intelligence briefing (curated content = stronger signal)
    BriefingClick,
    /// User dismissed the briefing without clicking any item
    BriefingDismiss,
    /// Deep engagement signal: user consumed content thoroughly
    EngagementComplete {
        total_seconds: u64,
        scroll_depth_pct: f32,
    },
    /// Save with explicit context — produces context-dependent decay & strength
    SaveWithContext {
        context: SaveContext,
    },
}

impl BehaviorAction {
    pub fn compute_strength(&self) -> f32 {
        match self {
            BehaviorAction::Click {
                dwell_time_seconds,
                pattern,
            } => {
                // Base dwell-only weight (legacy): 0.5 baseline + up to +0.5
                // from dwell. Range [0.5, 1.0].
                let base = 0.5;
                let dwell_bonus = (*dwell_time_seconds as f32 / 60.0).min(0.5);
                let legacy = base + dwell_bonus;

                match pattern {
                    // When the frontend classified the pattern, the pattern's
                    // multiplier OVERRIDES the legacy dwell-only weight. A
                    // bounce with 30s dwell (user stared confused then left)
                    // must not score as positive just because dwell was long.
                    Some(p) => legacy * p.strength_multiplier(),
                    None => legacy,
                }
            }
            BehaviorAction::Save => 1.0,
            BehaviorAction::Share => 1.0,
            BehaviorAction::Dismiss => -0.8,
            BehaviorAction::MarkIrrelevant => -1.0,
            BehaviorAction::Scroll { visible_seconds } => {
                // Log scale: 30s read ~ 0.52, 10s ~ 0.36, 2s ~ 0.16 (was capped at 0.30)
                0.15 * (1.0 + *visible_seconds).ln()
            }
            BehaviorAction::Ignore => -0.1,
            BehaviorAction::BriefingClick => 0.7, // Curated content click = stronger than general click
            BehaviorAction::BriefingDismiss => -0.2, // Mild negative — briefing wasn't useful today
            BehaviorAction::EngagementComplete {
                total_seconds,
                scroll_depth_pct,
            } => {
                let depth_factor = scroll_depth_pct.clamp(0.0, 1.0) * 0.4;
                let dwell_factor = (*total_seconds as f32 / 120.0).min(0.3);
                0.3 + depth_factor + dwell_factor // Range: 0.3 to 1.0
            }
            BehaviorAction::SaveWithContext { context } => match context {
                SaveContext::UsefulNow => 1.2,
                SaveContext::Reference => 0.9,
                SaveContext::Share => 1.0,
            },
        }
    }
}

/// Behavior signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSignal {
    pub item_id: i64,
    pub action: BehaviorAction,
    pub timestamp: String,
    pub item_topics: Vec<String>,
    pub item_source: String,
    pub signal_strength: f32,
}

/// Topic affinity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicAffinity {
    pub topic: String,
    pub embedding: Option<Vec<f32>>,
    pub positive_signals: u32,
    pub negative_signals: u32,
    pub total_exposures: u32,
    pub affinity_score: f32,
    pub confidence: f32,
    pub last_interaction: String,
    pub decay_applied: bool,
}

/// Anti-topic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiTopic {
    pub topic: String,
    pub rejection_count: u32,
    pub confidence: f32,
    pub auto_detected: bool,
    pub user_confirmed: bool,
    pub first_rejection: String,
    pub last_rejection: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_base_strength() {
        let action = BehaviorAction::Click {
            dwell_time_seconds: 0,
            pattern: None,
        };
        assert!((action.compute_strength() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_click_max_dwell_bonus() {
        let action = BehaviorAction::Click {
            dwell_time_seconds: 120,
            pattern: None,
        };
        // base 0.5 + min(120/60, 0.5) = 0.5 + 0.5 = 1.0
        assert!((action.compute_strength() - 1.0).abs() < f32::EPSILON);
    }

    // --- Interaction pattern tests ---
    //
    // The load-bearing claim these encode: a long dwell is NOT proof of
    // engagement. A bounced click (short + back) is actively negative even
    // if dwell appears moderate. Without these weights, the naive scorer
    // interprets confusion as interest and compounds errors.

    #[test]
    fn test_click_bounced_is_negative_despite_dwell() {
        // A user opens an item, struggles with it for 20s, backs out.
        // Naive weight: 0.5 + 20/60 ~ 0.83 (strong positive -- WRONG).
        // Pattern-aware: 0.83 * -0.4 = -0.33 (honest weak-negative).
        let action = BehaviorAction::Click {
            dwell_time_seconds: 20,
            pattern: Some(InteractionPattern::Bounced),
        };
        let strength = action.compute_strength();
        assert!(
            strength < 0.0,
            "bounced click must score negative, got {strength}"
        );
    }

    #[test]
    fn test_click_completed_is_stronger_than_naive_click() {
        // Same 30s dwell, but with Completed pattern (read to end, no return).
        // Naive: 0.5 + 30/60 = 1.0.
        // Completed multiplier: 1.0 * 1.4 = 1.4 -- higher ceiling.
        let completed = BehaviorAction::Click {
            dwell_time_seconds: 30,
            pattern: Some(InteractionPattern::Completed),
        };
        let naive = BehaviorAction::Click {
            dwell_time_seconds: 30,
            pattern: None,
        };
        assert!(completed.compute_strength() > naive.compute_strength());
    }

    #[test]
    fn test_click_abandoned_is_neutral() {
        // Very long dwell but Abandoned pattern (tab left open, no scroll).
        // Must NOT produce a positive signal just because dwell looks high.
        let action = BehaviorAction::Click {
            dwell_time_seconds: 600, // 10 minutes
            pattern: Some(InteractionPattern::Abandoned),
        };
        assert!((action.compute_strength() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_click_reread_signals_above_level() {
        // Re-reading patterns suggest the content was above the user's level.
        // Positive but muted, and flagged for necessity-scorer pickup.
        let action = BehaviorAction::Click {
            dwell_time_seconds: 60,
            pattern: Some(InteractionPattern::Reread),
        };
        let strength = action.compute_strength();
        assert!(strength > 0.0, "reread is still net-positive");
        assert!(strength < 1.0, "but less than a straight Engaged read");
        assert!(InteractionPattern::Reread.suggests_above_level());
    }

    #[test]
    fn test_engaged_and_completed_do_not_suggest_above_level() {
        assert!(!InteractionPattern::Engaged.suggests_above_level());
        assert!(!InteractionPattern::Completed.suggests_above_level());
        assert!(!InteractionPattern::Scanned.suggests_above_level());
        assert!(!InteractionPattern::Abandoned.suggests_above_level());
    }

    #[test]
    fn test_pattern_serde_snake_case_roundtrip() {
        // The frontend sends lowercase/snake_case strings; the deserializer
        // must round-trip without surprises. This pins the JSON wire format
        // for every variant so a rename here breaks the test visibly.
        for (variant, expected) in [
            (InteractionPattern::Bounced, "\"bounced\""),
            (InteractionPattern::Scanned, "\"scanned\""),
            (InteractionPattern::Engaged, "\"engaged\""),
            (InteractionPattern::Completed, "\"completed\""),
            (InteractionPattern::Reread, "\"reread\""),
            (InteractionPattern::Abandoned, "\"abandoned\""),
        ] {
            let serialized = serde_json::to_string(&variant).unwrap();
            assert_eq!(serialized, expected);
            let deserialized: InteractionPattern = serde_json::from_str(expected).unwrap();
            assert_eq!(deserialized, variant);
        }
    }

    #[test]
    fn test_save_strength() {
        let action = BehaviorAction::Save;
        assert!((action.compute_strength() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_share_strength() {
        let action = BehaviorAction::Share;
        assert!((action.compute_strength() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dismiss_strength() {
        let action = BehaviorAction::Dismiss;
        assert!((action.compute_strength() - (-0.8)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mark_irrelevant_strength() {
        let action = BehaviorAction::MarkIrrelevant;
        assert!((action.compute_strength() - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_scroll_strength() {
        let action = BehaviorAction::Scroll {
            visible_seconds: 3.0,
        };
        // Log scale: 0.15 * ln(1 + 3.0) ~ 0.2079
        let expected = 0.15 * (1.0 + 3.0_f32).ln();
        assert!((action.compute_strength() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_scroll_capped() {
        let action = BehaviorAction::Scroll {
            visible_seconds: 10.0,
        };
        // Log scale: 0.15 * ln(1 + 10.0) ~ 0.3598 (no hard cap, log naturally tapers)
        let expected = 0.15 * (1.0 + 10.0_f32).ln();
        assert!((action.compute_strength() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_ignore_strength() {
        let action = BehaviorAction::Ignore;
        assert!((action.compute_strength() - (-0.1)).abs() < f32::EPSILON);
    }

    // ========================================================================
    // Phase 2: Engagement Depth tests
    // ========================================================================

    #[test]
    fn test_engagement_complete_minimum() {
        // Zero scroll, zero time -> base 0.3
        let action = BehaviorAction::EngagementComplete {
            total_seconds: 0,
            scroll_depth_pct: 0.0,
        };
        assert!(
            (action.compute_strength() - 0.3).abs() < f32::EPSILON,
            "Min engagement should be 0.3, got {}",
            action.compute_strength()
        );
    }

    #[test]
    fn test_engagement_complete_maximum() {
        // Full scroll, 120+ seconds -> 0.3 + 0.4 + 0.3 = 1.0
        let action = BehaviorAction::EngagementComplete {
            total_seconds: 200,
            scroll_depth_pct: 1.0,
        };
        assert!(
            (action.compute_strength() - 1.0).abs() < f32::EPSILON,
            "Max engagement should be 1.0, got {}",
            action.compute_strength()
        );
    }

    #[test]
    fn test_engagement_complete_partial() {
        // 50% scroll, 60 seconds -> 0.3 + (0.5 * 0.4) + (60/120).min(0.3) = 0.3 + 0.2 + 0.15 = 0.65
        let action = BehaviorAction::EngagementComplete {
            total_seconds: 60,
            scroll_depth_pct: 0.5,
        };
        let expected = 0.3 + (0.5 * 0.4) + (60.0_f32 / 120.0).min(0.3);
        assert!(
            (action.compute_strength() - expected).abs() < 1e-6,
            "Partial engagement expected {}, got {}",
            expected,
            action.compute_strength()
        );
    }

    #[test]
    fn test_engagement_complete_clamps_scroll() {
        // scroll_depth_pct > 1.0 should clamp to 1.0
        let action = BehaviorAction::EngagementComplete {
            total_seconds: 0,
            scroll_depth_pct: 2.0,
        };
        // 0.3 + (1.0 * 0.4) + 0.0 = 0.7
        assert!(
            (action.compute_strength() - 0.7).abs() < f32::EPSILON,
            "Clamped scroll should give 0.7, got {}",
            action.compute_strength()
        );
    }

    // ========================================================================
    // Phase 2: SaveWithContext tests
    // ========================================================================

    #[test]
    fn test_save_with_context_useful_now() {
        let action = BehaviorAction::SaveWithContext {
            context: SaveContext::UsefulNow,
        };
        assert!(
            (action.compute_strength() - 1.2).abs() < f32::EPSILON,
            "UsefulNow should be 1.2, got {}",
            action.compute_strength()
        );
    }

    #[test]
    fn test_save_with_context_reference() {
        let action = BehaviorAction::SaveWithContext {
            context: SaveContext::Reference,
        };
        assert!(
            (action.compute_strength() - 0.9).abs() < f32::EPSILON,
            "Reference should be 0.9, got {}",
            action.compute_strength()
        );
    }

    #[test]
    fn test_save_with_context_share() {
        let action = BehaviorAction::SaveWithContext {
            context: SaveContext::Share,
        };
        assert!(
            (action.compute_strength() - 1.0).abs() < f32::EPSILON,
            "Share should be 1.0, got {}",
            action.compute_strength()
        );
    }

    // ========================================================================
    // Phase 2: Serde round-trip tests
    // ========================================================================

    #[test]
    fn test_engagement_complete_serde_roundtrip() {
        let action = BehaviorAction::EngagementComplete {
            total_seconds: 90,
            scroll_depth_pct: 0.75,
        };
        let json = serde_json::to_string(&action).expect("serialize");
        let deserialized: BehaviorAction = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_save_with_context_serde_roundtrip() {
        let action = BehaviorAction::SaveWithContext {
            context: SaveContext::Reference,
        };
        let json = serde_json::to_string(&action).expect("serialize");
        let deserialized: BehaviorAction = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(action, deserialized);
    }
}

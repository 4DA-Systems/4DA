// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior learning — user interaction tracking, topic affinities, anti-topics.

use rusqlite;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;

use super::ACE;

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
                // Log scale: 30s read ≈ 0.52, 10s ≈ 0.36, 2s ≈ 0.16 (was capped at 0.30)
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

/// Source preference (stub for API compatibility)
#[allow(dead_code)] // Reason: serde-deserialized struct; not yet constructed in production code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePreference {
    pub source: String,
    pub score: f32,
    pub interactions: u32,
}

/// Learned behavior (stub for API compatibility)
#[allow(dead_code)] // Reason: serde-deserialized struct; not yet constructed in production code
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearnedBehavior {
    pub interests: Vec<String>,
    pub anti_topics: Vec<String>,
}

/// Activity patterns (stub for API compatibility)
#[allow(dead_code)] // Reason: serde-deserialized struct; not yet constructed in production code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityPatterns {
    pub hourly_engagement: Vec<f32>,
    pub daily_engagement: Vec<f32>,
}

/// Summary of learned behavior
#[allow(dead_code)] // Reason: serde-deserialized struct; constructed by get_learned_behavior which is itself unused
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedBehaviorSummary {
    pub total_interactions: u32,
    pub learning_confidence: f32,
    pub interests: Vec<String>,
    pub anti_topics: Vec<String>,
    pub source_preferences: Vec<SourcePreferenceSummary>,
    pub top_affinities: Vec<TopicAffinity>,
}

/// Source preference summary
#[allow(dead_code)] // Reason: serde-deserialized struct; constructed by get_learned_behavior which is itself unused
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePreferenceSummary {
    pub source: String,
    pub score: f32,
    pub interactions: u32,
}

// ============================================================================
// ACE Behavior Methods
// ============================================================================

impl ACE {
    /// Record a user interaction
    pub fn record_interaction(
        &self,
        item_id: i64,
        action: BehaviorAction,
        item_topics: Vec<String>,
        item_source: String,
    ) -> Result<()> {
        if !self.rate_limiter.check(&item_source) {
            return Err("Rate limited: too many interactions".into());
        }

        let signal_strength = action.compute_strength();
        let signal = BehaviorSignal {
            item_id,
            action: action.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            item_topics: item_topics.clone(),
            item_source: item_source.clone(),
            signal_strength,
        };

        self.store_interaction(&signal)?;

        // Return-visit tracking: on click-like actions, increment view_count on source_items
        // and boost strength for return visits (view_count >= 2)
        let signal = if matches!(
            action,
            BehaviorAction::Click { .. }
                | BehaviorAction::BriefingClick
                | BehaviorAction::EngagementComplete { .. }
        ) {
            let view_count = self.increment_view_count(item_id).unwrap_or(0);
            if view_count >= 2 {
                // Return visit — user came back to this content, strong interest signal
                debug!(target: "ace::behavior",
                    item_id = item_id,
                    view_count = view_count,
                    "Return visit detected — boosting strength to 1.5"
                );
                BehaviorSignal {
                    signal_strength: 1.5,
                    ..signal
                }
            } else {
                signal
            }
        } else {
            signal
        };

        // Don't let security triage pollute topic learning.
        // Dismissing a CVE as "not applicable" shouldn't suppress future security content,
        // and saving a CVE shouldn't boost unrelated topics that happen to share keywords.
        let is_security_item = {
            let conn = self.conn.lock();
            conn.query_row(
                "SELECT necessity_category FROM item_necessity WHERE source_item_id = ?1",
                rusqlite::params![item_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap_or(None)
            .as_deref()
                == Some("security_vulnerability")
        };

        if !is_security_item {
            self.update_topic_affinities(&signal)?;

            if signal.signal_strength < -0.5 {
                self.update_anti_topics(&item_topics, signal.signal_strength)?;
            }
        } else {
            debug!(target: "ace::behavior",
                item_id = item_id,
                "Skipping affinity/anti-topic update for security vulnerability item"
            );
        }

        self.update_source_preference(&item_source, signal.signal_strength)?;
        self.update_activity_patterns(&signal)?;

        // Update continuous persona posterior from implicit signals
        if !item_topics.is_empty() {
            let conn = self.conn.lock();
            if let Err(e) = crate::taste_test::continuous::update_posterior(
                &conn,
                &item_topics,
                signal.signal_strength,
            ) {
                debug!(target: "ace::behavior", error = %e, "Failed to update continuous posterior");
            }
        }

        debug!(target: "ace::behavior",
            action = ?action,
            item_id = item_id,
            strength = signal.signal_strength,
            "Recorded behavior signal"
        );

        Ok(())
    }

    /// Update hourly and daily activity pattern counters
    fn update_activity_patterns(&self, signal: &BehaviorSignal) -> Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now();
        let hour = now.format("%H").to_string();
        let day = now.format("%A").to_string(); // Monday, Tuesday, etc.

        // Upsert hourly pattern
        conn.execute(
            "INSERT INTO activity_patterns (pattern_type, pattern_key, interaction_count, last_updated)
             VALUES ('hourly', ?1, 1, ?2)
             ON CONFLICT(pattern_type, pattern_key) DO UPDATE SET
                interaction_count = interaction_count + 1,
                last_updated = ?2",
            rusqlite::params![hour, signal.timestamp],
        )?;

        // Upsert daily pattern
        conn.execute(
            "INSERT INTO activity_patterns (pattern_type, pattern_key, interaction_count, last_updated)
             VALUES ('daily', ?1, 1, ?2)
             ON CONFLICT(pattern_type, pattern_key) DO UPDATE SET
                interaction_count = interaction_count + 1,
                last_updated = ?2",
            rusqlite::params![day, signal.timestamp],
        )?;

        Ok(())
    }

    /// Get rate limit status
    pub fn get_rate_limit_status(&self, source: &str) -> super::RateLimitStatus {
        self.rate_limiter.status(source)
    }

    fn store_interaction(&self, signal: &BehaviorSignal) -> Result<()> {
        let conn = self.conn.lock();

        let action_type = match &signal.action {
            BehaviorAction::Click { .. } => "click",
            BehaviorAction::Save => "save",
            BehaviorAction::Share => "share",
            BehaviorAction::Dismiss => "dismiss",
            BehaviorAction::MarkIrrelevant => "mark_irrelevant",
            BehaviorAction::Scroll { .. } => "scroll",
            BehaviorAction::Ignore => "ignore",
            BehaviorAction::BriefingClick => "briefing_click",
            BehaviorAction::BriefingDismiss => "briefing_dismiss",
            BehaviorAction::EngagementComplete { .. } => "engagement_complete",
            BehaviorAction::SaveWithContext { .. } => "save_with_context",
        };

        let action_data = serde_json::to_string(&signal.action).unwrap_or_default();
        let topics_json = serde_json::to_string(&signal.item_topics).unwrap_or_default();

        conn.execute(
            "INSERT INTO interactions (item_id, action_type, action_data, item_topics, item_source, signal_strength)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                signal.item_id,
                action_type,
                action_data,
                topics_json,
                signal.item_source,
                signal.signal_strength
            ],
        )?;

        Ok(())
    }

    /// Increment view_count on source_items and return the new count.
    /// Returns 0 if the item doesn't exist (no-op for non-existent items).
    fn increment_view_count(&self, item_id: i64) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE source_items SET view_count = COALESCE(view_count, 0) + 1 WHERE id = ?1",
            rusqlite::params![item_id],
        )?;
        let count: i64 = conn
            .query_row(
                "SELECT COALESCE(view_count, 0) FROM source_items WHERE id = ?1",
                rusqlite::params![item_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    fn update_topic_affinities(&self, signal: &BehaviorSignal) -> Result<()> {
        let conn = self.conn.lock();

        for topic in &signal.item_topics {
            if signal.signal_strength > 0.0 {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, positive_signals, total_exposures, last_interaction)
                     VALUES (?1, 1, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        positive_signals = topic_affinities.positive_signals + 1,
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        decay_applied = 0,
                        last_decay_at = NULL,
                        updated_at = datetime('now')",
                    rusqlite::params![topic],
                )
            } else if signal.signal_strength < 0.0 {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, negative_signals, total_exposures, last_interaction)
                     VALUES (?1, 1, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        negative_signals = topic_affinities.negative_signals + 1,
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        decay_applied = 0,
                        last_decay_at = NULL,
                        updated_at = datetime('now')",
                    rusqlite::params![topic],
                )
            } else {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, total_exposures, last_interaction)
                     VALUES (?1, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        updated_at = datetime('now')",
                    rusqlite::params![topic],
                )
            }?;

            // For strong negative signals (MarkIrrelevant = -1.0, Dismiss = -0.8),
            // activate affinity immediately — don't wait for 5 exposures.
            // Users expect instant feedback when they explicitly reject content.
            conn.execute(
                "UPDATE topic_affinities SET
                    affinity_score = CASE
                        WHEN negative_signals > 0 AND positive_signals = 0 THEN
                            -1.0 * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
                        WHEN total_exposures >= 3 THEN
                            (CAST(positive_signals AS REAL) - CAST(negative_signals AS REAL)) / CAST(total_exposures AS REAL)
                            * MIN(CAST(total_exposures AS REAL) / 20.0, 1.0)
                        ELSE 0.0
                    END,
                    confidence = CASE
                        WHEN negative_signals > 0 AND positive_signals = 0 THEN
                            MAX(0.3, MIN(CAST(total_exposures AS REAL) / 10.0, 1.0))
                        ELSE MIN(CAST(total_exposures AS REAL) / 20.0, 1.0)
                    END
                 WHERE topic = ?1",
                rusqlite::params![topic],
            )?;
        }

        Ok(())
    }

    fn update_anti_topics(&self, topics: &[String], signal_strength: f32) -> Result<()> {
        if signal_strength >= -0.5 {
            return Ok(());
        }

        let conn = self.conn.lock();

        for topic in topics {
            conn.execute(
                "INSERT INTO anti_topics (topic, rejection_count, confidence, last_rejection)
                 VALUES (?1, 1, 0.2, datetime('now'))
                 ON CONFLICT(topic) DO UPDATE SET
                    rejection_count = anti_topics.rejection_count + 1,
                    confidence = MIN(CAST(anti_topics.rejection_count + 1 AS REAL) / 10.0, 0.9),
                    last_rejection = datetime('now'),
                    updated_at = datetime('now')",
                rusqlite::params![topic],
            )?;
        }

        Ok(())
    }

    fn update_source_preference(&self, source: &str, signal_strength: f32) -> Result<()> {
        let conn = self.conn.lock();
        let alpha = 0.1;

        conn.execute(
            "INSERT INTO source_preferences (source, score, interactions, last_interaction)
             VALUES (?1, ?2, 1, datetime('now'))
             ON CONFLICT(source) DO UPDATE SET
                score = source_preferences.score * (1.0 - ?3) + ?2 * ?3,
                interactions = source_preferences.interactions + 1,
                last_interaction = datetime('now'),
                updated_at = datetime('now')",
            rusqlite::params![source, signal_strength, alpha],
        )?;

        Ok(())
    }

    /// Get topic affinities (default threshold: 5 exposures)
    pub fn get_topic_affinities(&self) -> Result<Vec<TopicAffinity>> {
        self.get_topic_affinities_min(5)
    }

    /// Get topic affinities with custom minimum exposure threshold.
    /// Use lower threshold (2-3) in bootstrap mode for faster learning.
    pub fn get_topic_affinities_min(&self, min_exposures: i64) -> Result<Vec<TopicAffinity>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT topic, positive_signals, negative_signals, total_exposures,
                    affinity_score, confidence, last_interaction
             FROM topic_affinities
             WHERE total_exposures >= ?1
             ORDER BY ABS(affinity_score) DESC
             LIMIT 100",
        )?;

        let rows = stmt.query_map([min_exposures], |row| {
            Ok(TopicAffinity {
                topic: row.get(0)?,
                embedding: None,
                positive_signals: row.get(1)?,
                negative_signals: row.get(2)?,
                total_exposures: row.get(3)?,
                affinity_score: row.get(4)?,
                confidence: row.get(5)?,
                last_interaction: row.get(6)?,
                decay_applied: false,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(std::convert::Into::into)
    }

    /// Get anti-topics
    pub fn get_anti_topics(&self, min_rejections: u32) -> Result<Vec<AntiTopic>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT topic, rejection_count, confidence, auto_detected, user_confirmed,
                    first_rejection, last_rejection
             FROM anti_topics
             WHERE rejection_count >= ?1
             ORDER BY rejection_count DESC",
        )?;

        let rows = stmt.query_map([min_rejections], |row| {
            Ok(AntiTopic {
                topic: row.get(0)?,
                rejection_count: row.get(1)?,
                confidence: row.get(2)?,
                auto_detected: row.get::<_, i32>(3)? != 0,
                user_confirmed: row.get::<_, i32>(4)? != 0,
                first_rejection: row.get(5)?,
                last_rejection: row.get(6)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(std::convert::Into::into)
    }

    /// Get behavior modifier for an item
    #[allow(dead_code)] // Reason: adaptive scoring feature not yet wired into scoring pipeline
    pub fn get_behavior_modifier(&self, topics: &[String], source: &str) -> Result<f32> {
        let conn = self.conn.lock();
        let mut modifier = 0.0;
        let mut count = 0;

        for topic in topics {
            let result: std::result::Result<(f32, f32), _> = conn.query_row(
                "SELECT affinity_score, confidence FROM topic_affinities WHERE topic = ?1",
                [topic],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );
            if let Ok((score, confidence)) = result {
                modifier += score * confidence;
                count += 1;
            }
        }

        if count > 0 {
            modifier /= count as f32;
        }

        let source_score: f32 = conn
            .query_row(
                "SELECT score FROM source_preferences WHERE source = ?1",
                [source],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        modifier += source_score * 0.3;

        Ok(modifier.clamp(-1.0, 1.0))
    }

    /// Get learned behavior summary
    #[allow(dead_code)] // Reason: adaptive scoring feature not yet wired into UI/commands
    pub fn get_learned_behavior(&self) -> Result<LearnedBehaviorSummary> {
        let affinities = self.get_topic_affinities()?;
        let anti_topics = self.get_anti_topics(5)?;

        let conn = self.conn.lock();

        let total_interactions: u32 = conn
            .query_row("SELECT COUNT(*) FROM interactions", [], |row| row.get(0))
            .unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT source, score, interactions FROM source_preferences ORDER BY score DESC",
        )?;

        let source_prefs: Vec<SourcePreferenceSummary> = stmt
            .query_map([], |row| {
                Ok(SourcePreferenceSummary {
                    source: row.get(0)?,
                    score: row.get(1)?,
                    interactions: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| -> crate::error::FourDaError { e.into() })?;

        let learning_confidence = (total_interactions as f32 / 100.0).min(0.95);

        let interests: Vec<String> = affinities
            .iter()
            .filter(|a| a.affinity_score > 0.3 && a.confidence > 0.5)
            .map(|a| a.topic.clone())
            .collect();

        Ok(LearnedBehaviorSummary {
            total_interactions,
            learning_confidence,
            interests,
            anti_topics: anti_topics.iter().map(|a| a.topic.clone()).collect(),
            source_preferences: source_prefs,
            top_affinities: affinities.into_iter().take(10).collect(),
        })
    }

    /// Get source preferences for scoring
    pub fn get_source_preferences(&self) -> Result<Vec<(String, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT source, score FROM source_preferences WHERE interactions >= 5 ORDER BY source",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(std::convert::Into::into)
    }

    /// Confirm an anti-topic
    #[allow(dead_code)] // Reason: anti-topic confirmation not yet exposed via UI/commands
    pub fn confirm_anti_topic(&self, topic: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE anti_topics SET user_confirmed = 1, confidence = 1.0, updated_at = datetime('now')
             WHERE topic = ?1",
            [topic],
        )?;
        Ok(())
    }

    /// Apply temporal decay to topic affinities
    /// Uses 30-day half-life: after 30 days of no interaction, scores halve.
    /// Runs continuously based on time since last decay (not a one-shot boolean).
    /// Deletes fully-decayed affinities (|score| < 0.05).
    pub fn apply_behavior_decay(&self) -> Result<usize> {
        let conn = self.conn.lock();

        // Fetch all affinities that haven't been interacted with in >1 day
        // Use last_decay_at to compute incremental decay (not decay from epoch)
        let mut stmt = conn.prepare(
            "SELECT topic, affinity_score, confidence, last_interaction,
                        COALESCE(last_decay_at, last_interaction) as decay_baseline
                 FROM topic_affinities
                 WHERE julianday('now') - julianday(last_interaction) > 1",
        )?;

        let rows: Vec<(String, f32, f32, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f32>(1)?,
                    row.get::<_, f32>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| -> crate::error::FourDaError { e.into() })?;

        let mut updated = 0;
        let now = chrono::Utc::now().to_rfc3339();

        for (topic, affinity_score, confidence, _last_interaction, decay_baseline) in &rows {
            // Parse the decay baseline timestamp
            let baseline = chrono::DateTime::parse_from_rfc3339(decay_baseline)
                .or_else(|_| {
                    // Try SQLite datetime format: "YYYY-MM-DD HH:MM:SS"
                    chrono::NaiveDateTime::parse_from_str(decay_baseline, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| dt.and_utc().fixed_offset())
                })
                .unwrap_or_else(|_| chrono::Utc::now().fixed_offset());

            let days_since = (chrono::Utc::now() - baseline.with_timezone(&chrono::Utc)).num_hours()
                as f32
                / 24.0;
            if days_since < 1.0 {
                continue; // Already decayed recently
            }

            // 30-day half-life decay
            let decay_factor = 0.5_f32.powf(days_since / 30.0);
            let new_affinity = affinity_score * decay_factor;
            let new_confidence = confidence.min(1.0) * decay_factor;

            // Delete fully-decayed affinities
            if new_affinity.abs() < 0.05 {
                conn.execute(
                    "DELETE FROM topic_affinities WHERE topic = ?1",
                    rusqlite::params![topic],
                )?;
                updated += 1;
                continue;
            }

            // Update with decayed values and record decay timestamp
            conn.execute(
                "UPDATE topic_affinities SET
                    affinity_score = ?1,
                    confidence = ?2,
                    last_decay_at = ?3,
                    decay_applied = 1
                 WHERE topic = ?4",
                rusqlite::params![new_affinity, new_confidence, now, topic],
            )?;

            updated += 1;
        }

        if updated > 0 {
            info!(target: "ace::behavior", updated = updated, "Applied temporal decay to topic affinities");
        }

        Ok(updated)
    }

    /// Apply temporal decay to detected technologies.
    /// Uses 60-day half-life (longer than topics since tech stacks change slower).
    /// Technologies below 0.15 confidence are removed.
    pub fn apply_detected_tech_decay(&self) -> Result<usize> {
        let conn = self.conn.lock();

        // Only decay entries not seen in >7 days (avoid decaying active projects)
        let mut stmt = conn.prepare(
            "SELECT name, category, confidence, last_seen
             FROM detected_tech
             WHERE julianday('now') - julianday(last_seen) > 7",
        )?;

        let rows: Vec<(String, String, f32, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f32>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| -> crate::error::FourDaError { e.into() })?;

        let mut updated = 0;

        for (name, _category, confidence, last_seen) in &rows {
            let baseline = chrono::DateTime::parse_from_rfc3339(last_seen)
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(last_seen, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| dt.and_utc().fixed_offset())
                })
                .unwrap_or_else(|_| chrono::Utc::now().fixed_offset());

            let days_since = (chrono::Utc::now() - baseline.with_timezone(&chrono::Utc)).num_hours()
                as f32
                / 24.0;

            if days_since < 7.0 {
                continue;
            }

            // 60-day half-life (tech stacks change slower than topic interests)
            let decay_factor = 0.5_f32.powf(days_since / 60.0);
            let new_confidence = confidence * decay_factor;

            if new_confidence < 0.15 {
                conn.execute(
                    "DELETE FROM detected_tech WHERE name = ?1",
                    rusqlite::params![name],
                )?;
            } else {
                conn.execute(
                    "UPDATE detected_tech SET confidence = ?1 WHERE name = ?2",
                    rusqlite::params![new_confidence, name],
                )?;
            }
            updated += 1;
        }

        if updated > 0 {
            info!(target: "ace::behavior", updated = updated, "Applied temporal decay to detected technologies");
        }

        Ok(updated)
    }
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
        // Naive weight: 0.5 + 20/60 ≈ 0.83 (strong positive — WRONG).
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
        // Completed multiplier: 1.0 * 1.4 = 1.4 — higher ceiling.
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
        // Log scale: 0.15 * ln(1 + 3.0) ≈ 0.2079
        let expected = 0.15 * (1.0 + 3.0_f32).ln();
        assert!((action.compute_strength() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_scroll_capped() {
        let action = BehaviorAction::Scroll {
            visible_seconds: 10.0,
        };
        // Log scale: 0.15 * ln(1 + 10.0) ≈ 0.3598 (no hard cap, log naturally tapers)
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
        // Zero scroll, zero time → base 0.3
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
        // Full scroll, 120+ seconds → 0.3 + 0.4 + 0.3 = 1.0
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
        // 50% scroll, 60 seconds → 0.3 + (0.5 * 0.4) + (60/120).min(0.3) = 0.3 + 0.2 + 0.15 = 0.65
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

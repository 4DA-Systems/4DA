// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior tracking — recording interactions, updating affinities, anti-topics, source prefs.

use rusqlite;
use tracing::debug;

use crate::ace::ACE;
use crate::error::Result;

use super::types::{BehaviorAction, BehaviorSignal};

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
    pub fn get_rate_limit_status(&self, source: &str) -> crate::ace::RateLimitStatus {
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
                            * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
                        ELSE 0.0
                    END,
                    confidence = CASE
                        WHEN negative_signals > 0 AND positive_signals = 0 THEN
                            MAX(0.3, MIN(CAST(total_exposures AS REAL) / 10.0, 1.0))
                        ELSE MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
                    END
                 WHERE topic = ?1",
                rusqlite::params![topic],
            )?;

            // Structured observability for the compound-learning loop. This is the
            // single source of truth for the "4DA gets sharper every day" promise —
            // every affinity change is traceable. Emitted on the "4da::learning"
            // target so it can be filtered independently of the noisier
            // ace::behavior debug stream and aggregated by get_learning_stats.
            if let Ok((score, confidence, exposures)) = conn.query_row(
                "SELECT affinity_score, confidence, total_exposures FROM topic_affinities WHERE topic = ?1",
                rusqlite::params![topic],
                |row| {
                    Ok((
                        row.get::<_, f32>(0)?,
                        row.get::<_, f32>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            ) {
                tracing::info!(
                    target: "4da::learning",
                    topic = %topic,
                    affinity_score = score,
                    confidence = confidence,
                    total_exposures = exposures,
                    signal_strength = signal.signal_strength,
                    "Topic affinity updated"
                );
            }
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
}

#[cfg(test)]
mod learning_loop_tests {
    use super::*;
    use crate::ace::create_test_ace;
    use crate::scoring::{compute_unified_relevance, ACEContext};

    /// End-to-end proof of the compound-learning loop: positive feedback on a
    /// topic must raise its affinity, negative feedback must lower it, and those
    /// learned affinities must then shift downstream scoring. This is the
    /// testable form of the "4DA gets sharper every day" promise — if it breaks,
    /// learning has silently stopped influencing the feed.
    #[test]
    fn feedback_shifts_affinities_and_scoring() {
        let ace = create_test_ace();

        // Positive feedback on three Rust items, negative on two Java items.
        for item_id in 1..=3 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Save,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            )
            .expect("record rust save");
        }
        for item_id in 4..=5 {
            ace.record_interaction(
                item_id,
                BehaviorAction::MarkIrrelevant,
                vec!["java".to_string()],
                "reddit".to_string(),
            )
            .expect("record java irrelevant");
        }

        // Read affinities through the same bootstrap path scoring uses
        // (min_exposures = 1 while feedback is sparse).
        let affinities = ace.get_topic_affinities_min(1).expect("read affinities");
        let rust = affinities
            .iter()
            .find(|a| a.topic == "rust")
            .expect("rust affinity present");
        let java = affinities
            .iter()
            .find(|a| a.topic == "java")
            .expect("java affinity present");

        assert!(
            rust.affinity_score > 0.05,
            "positive feedback should yield positive rust affinity, got {}",
            rust.affinity_score
        );
        assert!(
            java.affinity_score < -0.05,
            "negative feedback should yield negative java affinity, got {}",
            java.affinity_score
        );

        // Those learned affinities must move downstream scoring: a Rust item
        // should outscore an otherwise-identical Java item by a meaningful margin.
        let mut ctx = ACEContext::default();
        ctx.topic_affinities
            .insert("rust".to_string(), (rust.affinity_score, rust.confidence));
        ctx.topic_affinities
            .insert("java".to_string(), (java.affinity_score, java.confidence));

        let base = 0.5;
        let rust_score = compute_unified_relevance(base, &["rust".to_string()], &ctx);
        let java_score = compute_unified_relevance(base, &["java".to_string()], &ctx);

        assert!(
            rust_score > java_score,
            "learned affinity should rank rust above java ({rust_score} vs {java_score})"
        );
        assert!(
            rust_score - java_score >= 0.03,
            "learning effect should be meaningful, got margin {}",
            rust_score - java_score
        );
    }

    /// Saving a topic, then marking it irrelevant, should pull its affinity back
    /// down — the loop must respond to reversed feedback, not just accumulate.
    #[test]
    fn reversed_feedback_reverses_affinity() {
        let ace = create_test_ace();

        for item_id in 1..=3 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Save,
                vec!["kubernetes".to_string()],
                "hackernews".to_string(),
            )
            .expect("record save");
        }
        let positive = ace
            .get_topic_affinities_min(1)
            .expect("read")
            .into_iter()
            .find(|a| a.topic == "kubernetes")
            .expect("present")
            .affinity_score;
        assert!(positive > 0.0, "should start positive, got {positive}");

        for item_id in 4..=8 {
            ace.record_interaction(
                item_id,
                BehaviorAction::MarkIrrelevant,
                vec!["kubernetes".to_string()],
                "hackernews".to_string(),
            )
            .expect("record irrelevant");
        }
        let after = ace
            .get_topic_affinities_min(1)
            .expect("read")
            .into_iter()
            .find(|a| a.topic == "kubernetes")
            .expect("present")
            .affinity_score;

        assert!(
            after < positive,
            "reversed feedback should lower affinity ({after} should be < {positive})"
        );
    }
}

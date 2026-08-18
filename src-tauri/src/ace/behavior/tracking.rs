// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior tracking — recording explicit user interactions.
//!
//! v20b (AD-031): the implicit-capture layer that lived here — scroll/ignore
//! strength mapping, topic_affinities/anti_topics recompute, source_preferences,
//! activity_patterns, persona-posterior updates — was removed. This module now
//! records the raw `interactions` rows (explicit engagement + explicit
//! rejection) that the kept consumers read: skill-gap detection, the
//! engagement dashboard, bootstrap-mode counting, and autophagy calibration.

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
            item_topics,
            item_source: item_source.clone(),
            signal_strength,
        };

        self.store_interaction(&signal)?;

        // Return-visit tracking: on click-like actions, increment view_count on
        // source_items. (The former strength boost this fed was part of the
        // removed implicit-learning layer; the counter itself remains.)
        if matches!(
            action,
            BehaviorAction::Click { .. }
                | BehaviorAction::BriefingClick
                | BehaviorAction::EngagementComplete { .. }
        ) {
            let view_count = self.increment_view_count(item_id).unwrap_or(0);
            if view_count >= 2 {
                debug!(target: "ace::behavior",
                    item_id = item_id,
                    view_count = view_count,
                    "Return visit detected"
                );
            }
        }

        // Non-learning interactions (item_source prefixed "test_" or "probe_") verify the
        // recording MECHANICS without representing real engagement: the raw interaction
        // row is stored above, and read-side consumers (get_pro_value_report, skill-gap
        // detection) exclude or ignore these sources. This lets the founder dogfood
        // save/like/dismiss while keeping the instance VANILLA.
        let is_non_learning = item_source.starts_with("test_") || item_source.starts_with("probe_");
        if is_non_learning {
            debug!(target: "ace::behavior",
                item_id = item_id,
                source = %item_source,
                "Non-learning interaction: row stored (test/probe source)"
            );
        }

        debug!(target: "ace::behavior",
            action = ?action,
            item_id = item_id,
            strength = signal.signal_strength,
            "Recorded behavior signal"
        );

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
}

#[cfg(test)]
mod interaction_recording_tests {
    use super::*;
    use crate::ace::create_test_ace;

    /// A "test_"/"probe_"-sourced interaction still records its raw row — the
    /// save/dismiss mechanics stay verifiable — while read-side consumers
    /// exclude these sources so founder dogfooding never shapes the instance.
    /// (The profile-shift half of the pre-v20b twin test died with the
    /// implicit-learning layer: there is no profile left to shift.)
    #[test]
    fn non_learning_interaction_still_records_row() {
        let ace = create_test_ace();

        ace.record_interaction(
            999_999_999,
            BehaviorAction::Save,
            vec!["java".to_string()],
            "test_dogfood".to_string(),
        )
        .expect("non-learning save");

        let test_rows: i64 = {
            let conn = ace.get_conn().lock();
            conn.query_row(
                "SELECT COUNT(*) FROM interactions WHERE item_source = 'test_dogfood'",
                [],
                |r| r.get(0),
            )
            .expect("count test rows")
        };
        assert_eq!(test_rows, 1, "the test_ interaction must still be recorded");
    }

    /// Explicit engagement rows carry the action's computed strength into the
    /// interactions table — the raw signal the kept consumers (bootstrap
    /// counting, engagement dashboard, autophagy calibration) read.
    #[test]
    fn explicit_interaction_stores_strength_and_topics() {
        let ace = create_test_ace();

        ace.record_interaction(
            1,
            BehaviorAction::Save,
            vec!["rust".to_string()],
            "hackernews".to_string(),
        )
        .expect("record save");

        let (strength, topics): (f32, String) = {
            let conn = ace.get_conn().lock();
            conn.query_row(
                "SELECT signal_strength, item_topics FROM interactions WHERE item_id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("read row")
        };
        assert!(
            (strength - 1.0).abs() < f32::EPSILON,
            "Save strength is 1.0"
        );
        assert!(topics.contains("rust"), "topics JSON carries the topic");
    }
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior tracking — recording interactions, updating affinities, anti-topics, source prefs.

use rusqlite;
use tracing::debug;

use crate::ace::ACE;
use crate::error::Result;

use super::types::{BehaviorAction, BehaviorSignal};

/// Recompute one topic's affinity from its accumulated evidence (`?1` = topic).
///
/// Strength-weighted: `weighted_positive` / `weighted_negative` are sums of
/// |signal_strength|, so 40 passive ignores (−0.1 each) net a mild −4.0 while
/// three explicit dismissals (−0.8) net −2.4 over far fewer exposures — the
/// signal magnitude the user actually expressed survives into the score
/// (pre-2026-07-13 only COUNTS were compared, so passive and explicit
/// rejection weighed the same).
///
/// The instant-activation arm requires an EXPLICIT rejection
/// (`explicit_negative_signals`, strength <= −0.8) AND zero accumulated
/// positive evidence — a topic the user has only ever scrolled past can no
/// longer snap to −1.0, and one dismissal of a junk item cannot hard-poison a
/// topic the user has also engaged with positively (the check is on
/// `weighted_positive`, the backfilled truth, not the historical
/// `positive_signals` count, which pre-dates weighting and reads 0 for
/// evidence recorded before 2026-07-13).
///
/// Shared verbatim by the live per-interaction update and the profile-repair
/// migrations (Phase 89 backfill, Phase 90 arm correction) so all paths
/// produce identical scores.
pub(crate) const RECOMPUTE_AFFINITY_SQL: &str = "UPDATE topic_affinities SET
    affinity_score = CASE
        WHEN explicit_negative_signals > 0 AND weighted_positive <= 0.0 THEN
            -1.0 * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
        WHEN total_exposures >= 3 THEN
            MAX(-1.0, MIN(1.0,
                (weighted_positive - weighted_negative) / CAST(total_exposures AS REAL)
            )) * MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
        ELSE 0.0
    END,
    confidence = CASE
        WHEN explicit_negative_signals > 0 AND weighted_positive <= 0.0 THEN
            MAX(0.3, MIN(CAST(total_exposures AS REAL) / 10.0, 1.0))
        ELSE MIN(CAST(total_exposures AS REAL) / 10.0, 1.0)
    END
 WHERE topic = ?1";

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

        // Non-learning interactions (item_source prefixed "test_" or "probe_") verify the
        // recording MECHANICS without shifting the user's profile: the raw interaction row
        // was already stored above, but none of the learning below runs. This lets the
        // founder dogfood save/like/dismiss while keeping the instance VANILLA — model
        // quality is calibrated by the persona simulation (/calibrate), not by founder
        // engagement, so test clicks must never tailor scoring. Mirrors the probe_ exclusion
        // in get_pro_value_report.
        let is_non_learning = item_source.starts_with("test_") || item_source.starts_with("probe_");

        if is_non_learning {
            debug!(target: "ace::behavior",
                item_id = item_id,
                source = %item_source,
                "Non-learning interaction: row stored, profile NOT shifted (test/probe source)"
            );
        } else {
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

        // An explicit rejection (MarkIrrelevant −1.0, Dismiss −0.8) is a user
        // DECISION; a passive ignore (−0.1) is weak evidence. Only the former
        // may activate the instant negative arm below. Pre-2026-07-13 the
        // instant arm keyed on bare counts, so a topic whose only history was
        // passive ignores snapped to −1.0 at full confidence — the live
        // profile had typescript/tauri/rust/sqlite all at hard negative with
        // ZERO positive signals, suppressing the user's own stack at the
        // affinity multiplier's 0.3x floor.
        let is_explicit_negative = signal.signal_strength <= -0.8;

        for topic in &signal.item_topics {
            if signal.signal_strength > 0.0 {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, positive_signals, weighted_positive, total_exposures, last_interaction)
                     VALUES (?1, 1, ?2, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        positive_signals = topic_affinities.positive_signals + 1,
                        weighted_positive = topic_affinities.weighted_positive + ?2,
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        decay_applied = 0,
                        last_decay_at = NULL,
                        updated_at = datetime('now')",
                    rusqlite::params![topic, signal.signal_strength.min(1.5)],
                )
            } else if signal.signal_strength < 0.0 {
                conn.execute(
                    "INSERT INTO topic_affinities (topic, negative_signals, weighted_negative, explicit_negative_signals, total_exposures, last_interaction)
                     VALUES (?1, 1, ?2, ?3, 1, datetime('now'))
                     ON CONFLICT(topic) DO UPDATE SET
                        negative_signals = topic_affinities.negative_signals + 1,
                        weighted_negative = topic_affinities.weighted_negative + ?2,
                        explicit_negative_signals = topic_affinities.explicit_negative_signals + ?3,
                        total_exposures = topic_affinities.total_exposures + 1,
                        last_interaction = datetime('now'),
                        decay_applied = 0,
                        last_decay_at = NULL,
                        updated_at = datetime('now')",
                    rusqlite::params![
                        topic,
                        (-signal.signal_strength).min(1.5),
                        i64::from(is_explicit_negative)
                    ],
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

            conn.execute(RECOMPUTE_AFFINITY_SQL, rusqlite::params![topic])?;

            // Structured observability for the preference-profile capture loop:
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
    use crate::scoring::{compute_affinity_multiplier, ACEContext};

    /// End-to-end proof of the capture half of the learning loop: positive
    /// feedback on a topic must raise its affinity, negative feedback must
    /// lower it, and the learned values must flow to their remaining
    /// consumers (Learned Preferences panel, breakdown display,
    /// channel-render context).
    ///
    /// v19 (AD-029): learned affinities NO LONGER shift feed scoring — the
    /// scoring pipeline pins affinity_mult to 1.0 and the gate's learned
    /// axis never confirms (see pipeline_v2.rs and gate.rs). This test used
    /// to assert the opposite ("learned affinities must shift downstream
    /// scoring"); that assertion now runs against the legacy V1 helper only,
    /// as a guard that the capture→affinity→display chain stays alive for
    /// the surfaces that still read it.
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

        // The learned affinities must still move their DISPLAY consumer —
        // `compute_affinity_multiplier`, used by channel_render.rs — NOT the V2
        // feed pipeline, which pins affinity to neutral (AD-029).
        let mut ctx = ACEContext::default();
        ctx.topic_affinities
            .insert("rust".to_string(), (rust.affinity_score, rust.confidence));
        ctx.topic_affinities
            .insert("java".to_string(), (java.affinity_score, java.confidence));

        let base = 0.5;
        let rust_score = base * compute_affinity_multiplier(&["rust".to_string()], &ctx);
        let java_score = base * compute_affinity_multiplier(&["java".to_string()], &ctx);

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

    /// A "test_"/"probe_"-sourced interaction must NOT shift the profile — it keeps the
    /// founder's instance vanilla (calibration is persona-driven, not founder-driven) —
    /// yet the raw interaction row is STILL recorded so save/dismiss mechanics stay verifiable.
    #[test]
    fn non_learning_interaction_records_row_but_skips_profile() {
        let ace = create_test_ace();

        // Learning interaction on "rust" shifts affinity; test_ interaction on "java" must not.
        ace.record_interaction(
            1,
            BehaviorAction::Save,
            vec!["rust".to_string()],
            "hackernews".to_string(),
        )
        .expect("learning save");
        ace.record_interaction(
            999_999_999,
            BehaviorAction::Save,
            vec!["java".to_string()],
            "test_dogfood".to_string(),
        )
        .expect("non-learning save");

        let affinities = ace.get_topic_affinities_min(1).expect("read affinities");
        assert!(
            affinities.iter().any(|a| a.topic == "rust"),
            "the learning interaction must create a rust affinity"
        );
        assert!(
            !affinities.iter().any(|a| a.topic == "java"),
            "the test_ interaction must NOT shift the profile (no java affinity)"
        );

        // The raw interaction row IS still stored — mechanics remain verifiable.
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

    /// The 2026-07-13 doom loop: passive ignores alone must NEVER hard-poison
    /// a topic. Pre-fix, the instant negative arm keyed on bare counts, so a
    /// topic whose only history was −0.1 ignores snapped to −1.0 at full
    /// confidence (live profile: typescript −1.0, tauri −1.0, rust −0.84, all
    /// with ZERO positive signals — the user's own stack suppressed at the
    /// affinity multiplier's 0.3x floor).
    #[test]
    fn passive_ignores_cannot_hard_poison_a_topic() {
        let ace = create_test_ace();

        for item_id in 1..=10 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Ignore,
                vec!["tauri".to_string()],
                "crates_io".to_string(),
            )
            .expect("record ignore");
        }

        let affinities = ace.get_topic_affinities_min(1).expect("read affinities");
        let tauri = affinities
            .iter()
            .find(|a| a.topic == "tauri")
            .expect("tauri affinity present");
        // 10 ignores × −0.1 = weighted_negative 1.0 over 10 exposures →
        // affinity −0.1 × exposure-conf 1.0 = −0.10. Mildly negative is
        // correct (the user did skip it); hard-negative is the bug.
        assert!(
            tauri.affinity_score > -0.25,
            "passive ignores must stay mild, got {}",
            tauri.affinity_score
        );
        assert!(
            tauri.affinity_score <= 0.0,
            "ignores are still (weak) negative evidence, got {}",
            tauri.affinity_score
        );
    }

    /// Explicit rejection keeps its instant-activation teeth: one
    /// MarkIrrelevant must move the topic negative immediately with real
    /// confidence — users expect explicit feedback to bite without waiting
    /// for exposure accumulation.
    #[test]
    fn explicit_rejection_still_activates_instantly() {
        let ace = create_test_ace();

        ace.record_interaction(
            1,
            BehaviorAction::MarkIrrelevant,
            vec!["blockchain".to_string()],
            "hackernews".to_string(),
        )
        .expect("record mark_irrelevant");

        let affinities = ace.get_topic_affinities_min(1).expect("read affinities");
        let topic = affinities
            .iter()
            .find(|a| a.topic == "blockchain")
            .expect("affinity present");
        assert!(
            topic.affinity_score < 0.0,
            "explicit rejection must go negative immediately, got {}",
            topic.affinity_score
        );
        assert!(
            topic.confidence >= 0.3,
            "explicit rejection carries immediate confidence, got {}",
            topic.confidence
        );
    }

    /// The live `rust` residual (2026-07-14): ONE explicit dismissal of a
    /// junk item must not hard-poison a topic that also carries positive
    /// weighted evidence — the instant arm keys on weighted_positive (the
    /// backfilled truth), and the weighted formula takes over.
    #[test]
    fn one_explicit_rejection_does_not_hard_poison_engaged_topic() {
        let ace = create_test_ace();

        // Real positive engagement first…
        ace.record_interaction(
            1,
            BehaviorAction::Click {
                dwell_time_seconds: 30,
                pattern: None,
            },
            vec!["rust".to_string()],
            "hackernews".to_string(),
        )
        .expect("click");
        // …then one explicit rejection (of, say, a junk item titled with rust).
        ace.record_interaction(
            2,
            BehaviorAction::Dismiss,
            vec!["rust".to_string()],
            "crates_io".to_string(),
        )
        .expect("dismiss");
        // Some passive exposures so the weighted arm is active (>= 3).
        ace.record_interaction(
            3,
            BehaviorAction::Ignore,
            vec!["rust".to_string()],
            "hackernews".to_string(),
        )
        .expect("ignore");

        let affinities = ace.get_topic_affinities_min(1).expect("read affinities");
        let rust = affinities
            .iter()
            .find(|a| a.topic == "rust")
            .expect("rust affinity present");
        assert!(
            rust.affinity_score > -0.5,
            "one dismissal must not hard-poison an engaged topic, got {}",
            rust.affinity_score
        );
    }

    /// The strength-weighted formula keeps magnitudes honest when positives
    /// and passive negatives mix: a topic with real engagement plus a few
    /// scroll-pasts must stay net-positive.
    #[test]
    fn mixed_engagement_stays_net_positive() {
        let ace = create_test_ace();

        for item_id in 1..=2 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Save,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            )
            .expect("save");
        }
        for item_id in 3..=6 {
            ace.record_interaction(
                item_id,
                BehaviorAction::Ignore,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            )
            .expect("ignore");
        }

        let affinities = ace.get_topic_affinities_min(1).expect("read affinities");
        let rust = affinities
            .iter()
            .find(|a| a.topic == "rust")
            .expect("rust affinity present");
        // weighted: +2.0 vs −0.4 over 6 exposures → clearly positive.
        assert!(
            rust.affinity_score > 0.05,
            "2 saves must outweigh 4 passive ignores, got {}",
            rust.affinity_score
        );
    }
}

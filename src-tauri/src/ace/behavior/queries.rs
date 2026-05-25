// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior queries — reading topic affinities, anti-topics, source preferences.

use crate::ace::ACE;
use crate::error::Result;

use super::types::{AntiTopic, TopicAffinity};

impl ACE {
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
}

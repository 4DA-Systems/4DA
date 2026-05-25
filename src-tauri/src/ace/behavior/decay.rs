// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Temporal decay — half-life decay for topic affinities and detected technologies.

use rusqlite;
use tracing::info;

use crate::ace::ACE;
use crate::error::Result;

impl ACE {
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

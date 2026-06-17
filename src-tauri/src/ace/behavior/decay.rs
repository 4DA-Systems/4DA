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

        // detected_tech has no `last_seen` column — `updated_at` is the real
        // "last detected" signal (refreshed by every re-detection upsert). Only
        // decay tech not re-detected in >7 days (avoid decaying active projects).
        // The decay AMOUNT is measured from COALESCE(last_decay_at, updated_at) so
        // repeated daily runs re-baseline instead of re-applying the full elapsed
        // factor to an already-decayed value (the compounding bug). Mirrors the
        // sibling apply_behavior_decay exactly.
        let mut stmt = conn.prepare(
            "SELECT name, category, confidence,
                    COALESCE(last_decay_at, updated_at) as decay_baseline
             FROM detected_tech
             WHERE julianday('now') - julianday(updated_at) > 7",
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
        let now = chrono::Utc::now().to_rfc3339();

        for (name, _category, confidence, decay_baseline) in &rows {
            let baseline = chrono::DateTime::parse_from_rfc3339(decay_baseline)
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(decay_baseline, "%Y-%m-%d %H:%M:%S")
                        .map(|dt| dt.and_utc().fixed_offset())
                })
                .unwrap_or_else(|_| chrono::Utc::now().fixed_offset());

            let days_since = (chrono::Utc::now() - baseline.with_timezone(&chrono::Utc)).num_hours()
                as f32
                / 24.0;

            if days_since < 1.0 {
                continue; // Already decayed recently — wait for measurable elapsed time
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
                // Record the decay timestamp so the next run measures from NOW,
                // not from updated_at again (prevents quadratic compounding).
                conn.execute(
                    "UPDATE detected_tech SET confidence = ?1, last_decay_at = ?2 WHERE name = ?3",
                    rusqlite::params![new_confidence, now, name],
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
    use crate::ace::create_test_ace;

    fn tech_confidence(ace: &crate::ace::ACE, name: &str) -> Option<f32> {
        let conn = ace.conn.lock();
        conn.query_row(
            "SELECT confidence FROM detected_tech WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get::<_, f32>(0),
        )
        .ok()
    }

    /// Bug G regression: detected_tech decay must (1) actually run — the table has no
    /// `last_seen` column, so the old query errored and decay never executed — and
    /// (2) not compound. A second run immediately after the first must leave the
    /// confidence unchanged, because `last_decay_at` re-baselines the elapsed time.
    #[test]
    fn detected_tech_decay_runs_and_does_not_compound() {
        let ace = create_test_ace();
        {
            let conn = ace.conn.lock();
            conn.execute(
                "INSERT INTO detected_tech (name, category, confidence, source, evidence, updated_at)
                 VALUES ('rust', 'language', 0.9, 'manifest', 'Cargo.toml', datetime('now','-40 days'))",
                [],
            )
            .unwrap();
        }

        // First run: actually decays (proves the query no longer errors on last_seen).
        let n1 = ace
            .apply_detected_tech_decay()
            .expect("decay must not error");
        assert_eq!(n1, 1, "the stale tech row should be decayed");
        let after_first = tech_confidence(&ace, "rust").expect("rust still present");
        assert!(
            after_first < 0.9 && after_first > 0.15,
            "decayed but not deleted, got {after_first}"
        );

        // Second run immediately after: last_decay_at is ~now, so days_since < 1 and
        // nothing decays. With the compounding bug this would re-apply the full
        // 40-day factor and shrink the value again.
        let n2 = ace
            .apply_detected_tech_decay()
            .expect("decay must not error");
        assert_eq!(n2, 0, "no re-decay within a day (no compounding)");
        let after_second = tech_confidence(&ace, "rust").expect("rust still present");
        assert_eq!(
            after_first, after_second,
            "confidence must be unchanged on an immediate second run"
        );
    }
}

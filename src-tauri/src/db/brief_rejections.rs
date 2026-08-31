// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Persistence for Brief rejection verdicts (`brief_rejections` table).
//!
//! Written by the briefing pipeline after parsing the machine trailer
//! (see `crate::brief_rejections`), read by the scoring analyzer to demote
//! feed items the narrated Brief already rejected. Internal plumbing only —
//! not a UI intelligence type (doctrine rule 1 untouched).

use std::collections::HashMap;

use rusqlite::{params, Result as SqliteResult};

use super::Database;

/// Rejections older than this are dead weight: the feed only reads a 7-day
/// window, so a 30-day retention leaves generous forensic margin.
const BRIEF_REJECTION_RETENTION_DAYS: u32 = 30;

impl Database {
    /// Record the Brief's structured rejection verdicts for one briefing.
    /// Also prunes rows past the retention window so the table stays bounded.
    pub fn save_brief_rejections(
        &self,
        briefing_id: i64,
        rejections: &[(i64, String)],
    ) -> SqliteResult<usize> {
        if rejections.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO brief_rejections (briefing_id, source_item_id, reason)
                 VALUES (?1, ?2, ?3)",
            )?;
            for (item_id, reason) in rejections {
                stmt.execute(params![briefing_id, item_id, reason])?;
            }
        }
        tx.execute(
            "DELETE FROM brief_rejections
             WHERE created_at < datetime('now', ?1)",
            params![format!("-{BRIEF_REJECTION_RETENTION_DAYS} days")],
        )?;
        tx.commit()?;
        Ok(rejections.len())
    }

    /// The LATEST briefing's rejection verdicts, with that briefing's age in
    /// hours (AD-035 display binding).
    ///
    /// Returns `None` when no briefing has ever been persisted. The verdict
    /// list is EMPTY when the newest briefing recorded no verdicts (e.g. the
    /// deterministic floor, or a fail-open trailer parse) — an older
    /// briefing's verdicts never bind once a newer briefing exists: one item,
    /// one verdict, and the latest judgment is the judgment.
    pub fn get_latest_brief_verdicts(&self) -> SqliteResult<Option<(f64, Vec<(i64, String)>)>> {
        let conn = self.read_conn();
        let latest: Option<(i64, f64)> = conn
            .query_row(
                "SELECT id, (julianday('now') - julianday(created_at)) * 24.0
                 FROM briefings ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let Some((briefing_id, age_hours)) = latest else {
            return Ok(None);
        };
        let mut stmt = conn.prepare(
            "SELECT source_item_id, reason FROM brief_rejections
             WHERE briefing_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![briefing_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut verdicts = Vec::new();
        for row in rows {
            verdicts.push(row?);
        }
        Ok(Some((age_hours, verdicts)))
    }

    /// Rejection verdicts from the last `days` days, keyed by source item id.
    /// When an item was rejected more than once, the most recent reason wins.
    pub fn get_recent_brief_rejections(&self, days: u32) -> SqliteResult<HashMap<i64, String>> {
        let conn = self.read_conn();
        let mut stmt = conn.prepare(
            "SELECT source_item_id, reason FROM brief_rejections
             WHERE created_at >= datetime('now', ?1)
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![format!("-{days} days")], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (id, reason) = row?;
            map.insert(id, reason);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::test_db;

    #[test]
    fn save_and_get_roundtrip() {
        let db = test_db();
        let saved = db
            .save_brief_rejections(
                1,
                &[
                    (100, "self-promotional".to_string()),
                    (200, "no stack relevance".to_string()),
                ],
            )
            .expect("save rejections");
        assert_eq!(saved, 2);

        let recent = db.get_recent_brief_rejections(7).expect("read rejections");
        assert_eq!(recent.len(), 2);
        assert_eq!(
            recent.get(&100).map(String::as_str),
            Some("self-promotional")
        );
        assert_eq!(
            recent.get(&200).map(String::as_str),
            Some("no stack relevance")
        );
    }

    #[test]
    fn empty_save_is_a_noop() {
        let db = test_db();
        assert_eq!(db.save_brief_rejections(1, &[]).expect("save"), 0);
        assert!(db.get_recent_brief_rejections(7).expect("read").is_empty());
    }

    #[test]
    fn rejections_older_than_window_are_ignored() {
        let db = test_db();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO brief_rejections (briefing_id, source_item_id, reason, created_at)
                 VALUES (1, 100, 'stale verdict', datetime('now', '-8 days'))",
                [],
            )
            .expect("insert stale rejection");
        }
        db.save_brief_rejections(2, &[(200, "fresh verdict".to_string())])
            .expect("save fresh");

        let recent = db.get_recent_brief_rejections(7).expect("read");
        assert_eq!(recent.len(), 1, ">7-day-old rejection must be ignored");
        assert_eq!(recent.get(&200).map(String::as_str), Some("fresh verdict"));
        assert!(!recent.contains_key(&100));
    }

    #[test]
    fn latest_brief_verdicts_returns_newest_briefings_verdicts_with_age() {
        let db = test_db();
        let old_id = db
            .save_briefing("## Old brief", Some("m"), 3, Some(0), Some(0))
            .expect("save old");
        db.save_brief_rejections(old_id, &[(100, "old verdict".to_string())])
            .expect("save old verdicts");
        // Backdate the old briefing so ordering is unambiguous.
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE briefings SET created_at = datetime('now', '-2 days') WHERE id = ?1",
                rusqlite::params![old_id],
            )
            .expect("backdate");
        }
        let new_id = db
            .save_briefing("## Fresh brief", Some("m"), 5, Some(0), Some(0))
            .expect("save new");
        db.save_brief_rejections(
            new_id,
            &[
                (200, "self-promotional".to_string()),
                (300, "no stack relevance".to_string()),
            ],
        )
        .expect("save new verdicts");

        let (age_hours, verdicts) = db
            .get_latest_brief_verdicts()
            .expect("read")
            .expect("a briefing exists");
        assert!(age_hours >= 0.0 && age_hours < 0.1, "fresh briefing age");
        assert_eq!(
            verdicts,
            vec![
                (200, "self-promotional".to_string()),
                (300, "no stack relevance".to_string()),
            ],
            "only the LATEST briefing's verdicts are returned"
        );
    }

    #[test]
    fn latest_brief_verdicts_newer_verdictless_briefing_unbinds_older_verdicts() {
        // A deterministic-floor brief (no trailer, no verdicts) saved AFTER an
        // LLM brief must supersede it: latest briefing binds, and it recorded
        // nothing — so nothing binds.
        let db = test_db();
        let llm_id = db
            .save_briefing("## LLM brief", Some("claude"), 5, Some(0), Some(0))
            .expect("save llm");
        db.save_brief_rejections(llm_id, &[(100, "spam".to_string())])
            .expect("save verdicts");
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE briefings SET created_at = datetime('now', '-1 hour') WHERE id = ?1",
                rusqlite::params![llm_id],
            )
            .expect("backdate");
        }
        db.save_briefing("## Floor", Some("deterministic"), 2, Some(0), Some(0))
            .expect("save floor");

        let (_, verdicts) = db
            .get_latest_brief_verdicts()
            .expect("read")
            .expect("briefings exist");
        assert!(
            verdicts.is_empty(),
            "a newer verdict-less briefing must unbind the older one's verdicts"
        );
    }

    #[test]
    fn latest_brief_verdicts_none_without_any_briefing() {
        let db = test_db();
        assert!(db.get_latest_brief_verdicts().expect("read").is_none());
    }

    #[test]
    fn most_recent_reason_wins_for_repeat_rejections() {
        let db = test_db();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO brief_rejections (briefing_id, source_item_id, reason, created_at)
                 VALUES (1, 100, 'old reason', datetime('now', '-2 days'))",
                [],
            )
            .expect("insert older rejection");
        }
        db.save_brief_rejections(2, &[(100, "new reason".to_string())])
            .expect("save newer");

        let recent = db.get_recent_brief_rejections(7).expect("read");
        assert_eq!(recent.get(&100).map(String::as_str), Some("new reason"));
    }
}

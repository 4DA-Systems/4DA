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

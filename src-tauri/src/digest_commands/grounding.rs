// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! When 4DA first held an advisory: the only honest age a brief may state.
//!
//! Split from `digest_commands.rs` (file-size gate). The security facts
//! themselves — fix path, worst tier, scope — are built by `brief_facts`
//! from the same Preemption feed the tab reads (Decision 2, 2026-10-02);
//! this module answers the one question those facts ask of the source rows.

/// `(YYYY-MM-DD, days ago)` of the earliest osv/cve source row naming one of
/// `ids`, or `None` when no such row exists — then the brief carries no age
/// at all. The model otherwise invented one and incremented it every brief
/// (2026-09-07: "past day 20", 21, 22, 23, 24 across five briefs).
pub(crate) fn first_seen_for_ids(
    conn: &rusqlite::Connection,
    ids: &[String],
) -> Option<(String, i64)> {
    let mut earliest: Option<String> = None;
    let mut stmt = conn
        .prepare(
            "SELECT MIN(created_at) FROM source_items
             WHERE source_type IN ('osv', 'cve') AND title LIKE '%' || ?1 || '%'",
        )
        .ok()?;
    for id in ids {
        let found: Option<String> = stmt
            .query_row(rusqlite::params![id], |r| r.get::<_, Option<String>>(0))
            .ok()
            .flatten();
        if let Some(ts) = found {
            if earliest.as_ref().is_none_or(|e| ts < *e) {
                earliest = Some(ts);
            }
        }
    }
    let ts = earliest?;
    let date = ts.get(..10)?.to_string();
    let seen = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?;
    let days = (chrono::Utc::now().date_naive() - seen).num_days().max(0);
    Some((date, days))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{insert_test_item, test_db};

    /// The only honest age is when 4DA first held the advisory; a brief that
    /// cannot find one carries no age and the prompt rule forbids inventing it.
    #[test]
    fn first_seen_is_the_earliest_advisory_row_naming_the_id() {
        let db = test_db();
        insert_test_item(
            &db,
            "osv",
            "a1",
            "[GHSA-h395-gr6q-cpjc] jsonwebtoken: type confusion",
            "body",
        );
        let conn = db.conn.lock();
        let (date, days) = first_seen_for_ids(&conn, &["GHSA-h395-gr6q-cpjc".to_string()])
            .expect("an osv row names the id");
        assert_eq!(date, chrono::Utc::now().date_naive().to_string());
        assert_eq!(days, 0);
        assert!(
            first_seen_for_ids(&conn, &["GHSA-nope-nope-nope".to_string()]).is_none(),
            "no row, no age"
        );
    }
}

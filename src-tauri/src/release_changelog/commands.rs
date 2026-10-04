// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! IPC for the release card: "what changed" for one feed item.

use rusqlite::{params, OptionalExtension};

use super::{changelog_for, release_target, summarize, unavailable, ReleaseChanges};
use crate::error::Result;
use crate::get_database;

/// What changed between the installed and the announced version of a graded
/// registry release row. `None` for any other item (not a registry release,
/// or a release the grade does not call news). Reads the cache; reads the
/// package archive from the registry when the background lane has not yet.
#[tauri::command]
pub async fn get_release_changes(item_id: i64) -> Result<Option<ReleaseChanges>> {
    let db = get_database()?;
    let row: Option<(String, String, String)> = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT source_type, title, COALESCE(content, '') FROM source_items WHERE id = ?1",
            params![item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
    };
    let Some((source_type, title, content)) = row else {
        return Ok(None);
    };
    let Some(target) = release_target(db, &source_type, &title, &content) else {
        return Ok(None);
    };
    Ok(Some(match changelog_for(db, &target).await {
        Ok(record) => summarize(&target, &record),
        Err(reason) => unavailable(&target, reason),
    }))
}

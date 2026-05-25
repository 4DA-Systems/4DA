// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Row mapping helpers for SQLite result conversion.

use super::types::{DependencyAlert, StoredDependency};

pub(crate) fn map_dependency_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredDependency> {
    Ok(StoredDependency {
        id: row.get(0)?,
        project_path: row.get(1)?,
        package_name: row.get(2)?,
        version: row.get(3)?,
        ecosystem: row.get(4)?,
        is_dev: row.get::<_, i32>(5)? != 0,
        is_direct: row.get::<_, i32>(6)? != 0,
        detected_at: row.get(7)?,
        last_seen_at: row.get(8)?,
        license: row.get(9)?,
    })
}

pub(crate) fn map_alert_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DependencyAlert> {
    Ok(DependencyAlert {
        id: row.get(0)?,
        package_name: row.get(1)?,
        ecosystem: row.get(2)?,
        alert_type: row.get(3)?,
        severity: row.get(4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        affected_versions: row.get(7)?,
        source_url: row.get(8)?,
        source_item_id: row.get(9)?,
        detected_at: row.get(10)?,
        resolved_at: row.get(11)?,
    })
}

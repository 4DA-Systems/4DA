// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Decision-window auto-validation closer (ADR §3, I-4).
//!
//! Measures 4DA's core preemption claim — "we warned you N hours early" — by
//! checking each open `security_patch`/`migration` window against reality: the
//! earliest dependency-grounded incident event that occurred AFTER the window
//! opened. When found, the window is closed as `validated` with the measured
//! lead time and a `preemption_wins` row is recorded.
//!
//! Grounding is non-negotiable (ADR §3.4): a win is only minted from an event
//! that matches the window's OWN dependency (parameterized LIKE) and the
//! predicted event type. False wins are the cardinal sin.

use rusqlite::{params, Connection};
use tracing::{info, warn};

use super::DecisionWindow;
use crate::error::Result;

/// Validate open decision windows against reality. For each open security_patch/
/// migration window, find the earliest dep-grounded "incident" event that occurred
/// AFTER the window opened; if found, close the window as validated with the measured
/// lead time and record a preemption_wins row. Returns the number of wins recorded.
pub(crate) fn validate_open_windows(conn: &Connection) -> i64 {
    // Self-healing first: purge any previously recorded win whose dep-grounding
    // would no longer pass the ambiguity guard (false wins are the cardinal sin).
    revalidate_recorded_wins(conn);
    let mut wins = 0i64;
    for w in super::get_open_windows(conn) {
        match validate_single_window(conn, &w) {
            Ok(true) => wins += 1,
            Ok(false) => {}
            Err(e) => {
                // One bad window must not abort the pass.
                warn!(
                    target: "4da::decision_advantage",
                    id = w.id, error = %e,
                    "Window validation skipped (error)"
                );
            }
        }
    }
    if wins > 0 {
        info!(target: "4da::decision_advantage", wins, "Decision windows validated");
    }
    wins
}

/// Validate a single window. Returns Ok(true) when a preemption win was recorded.
fn validate_single_window(conn: &Connection, w: &DecisionWindow) -> Result<bool> {
    // Only security_patch / migration are reality-mappable in v1.
    // adoption is deferred (fuzzy, prone to false wins); knowledge has no event.
    if !matches!(w.window_type.as_str(), "security_patch" | "migration") {
        return Ok(false);
    }

    // Grounding requires a dependency.
    let dep = match w.dependency.as_deref() {
        Some(d) if !d.trim().is_empty() => d.to_lowercase(),
        _ => return Ok(false),
    };

    // Self-trigger exclusion: the items that OPENED the window can't be the incident.
    let excluded = excluded_source_item_ids(conn, w.id);

    let incident = match find_incident(conn, w, &dep, &excluded)? {
        Some(i) => i,
        None => return Ok(false),
    };

    // Lead time = incident − opened (hours). Must be strictly positive.
    let lead_time_hours = match compute_lead_time(&w.opened_at, &incident.created_at) {
        Some(h) if h > 0.0 => h,
        _ => return Ok(false),
    };

    // Close the window as validated. Do NOT reuse transition_window — it computes
    // lead as now−opened, which is wrong for a measured historical incident.
    conn.execute(
        "UPDATE decision_windows SET status = 'closed', closed_at = datetime('now'), \
         outcome = 'validated', lead_time_hours = ?1 WHERE id = ?2",
        params![lead_time_hours, w.id],
    )?;

    // Record the preemption win (verified = 1).
    conn.execute(
        "INSERT INTO preemption_wins \
         (alert_id, alert_title, alerted_at, incident_at, lead_time_hours, affected_deps, verified) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![
            w.id.to_string(),
            w.title,
            w.opened_at,
            incident.created_at,
            lead_time_hours,
            w.dependency,
        ],
    )?;

    info!(
        target: "4da::decision_advantage",
        id = w.id, dep = %dep, lead_time_hours,
        "Decision window validated against incident"
    );
    Ok(true)
}

/// An incident event from source_items that validates a window.
struct Incident {
    created_at: String,
}

/// Read + parse the JSON int array of source_item_ids attached to the window.
fn excluded_source_item_ids(conn: &Connection, window_id: i64) -> Vec<i64> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT source_item_ids FROM decision_windows WHERE id = ?1",
            params![window_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    match raw {
        Some(s) => serde_json::from_str::<Vec<i64>>(&s).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Find the earliest dep-grounded incident in source_items that occurred AFTER the
/// window opened and matches the predicted event type. Returns None when none qualify.
fn find_incident(
    conn: &Connection,
    w: &DecisionWindow,
    dep: &str,
    excluded: &[i64],
) -> Result<Option<Incident>> {
    // Event-type clause by window_type.
    let event_clause = match w.window_type.as_str() {
        "security_patch" => "(content_type IN ('cve', 'security_advisory') OR cve_ids IS NOT NULL)",
        "migration" => {
            "(signal_type = 'breaking_change' OR content_type IN ('release_notes', 'platform_update'))"
        }
        _ => return Ok(None),
    };

    // Exclusion clause — skip entirely when nothing to exclude.
    let exclude_clause = if excluded.is_empty() {
        String::new()
    } else {
        let placeholders = excluded
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("AND id NOT IN ({placeholders})")
    };

    // Dep grounding happens in Rust via the shared ambiguity guard - a SQL
    // LIKE '%dep%' matches raw substrings ("os" inside "close"/"macos") and
    // minted false wins. Pull candidate events, earliest first, and return
    // the first that genuinely talks about this dependency.
    let sql = format!(
        "SELECT title, COALESCE(content, ''), created_at FROM source_items \
         WHERE created_at > ?1 \
           AND {event_clause} \
           {exclude_clause} \
         ORDER BY created_at ASC LIMIT 500"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![w.opened_at], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (title, content, created_at) = row?;
        if crate::package_ambiguity::dep_grounded_match(
            &title.to_lowercase(),
            &content.to_lowercase(),
            dep,
        ) {
            return Ok(Some(Incident { created_at }));
        }
    }
    Ok(None)
}

/// Self-healing sweep over already-recorded verified wins: delete any row whose
/// dependency is a strict-proof name (ambiguous or <4 chars) that does NOT
/// appear at a word boundary in the SOURCE portion of the alert title. Window
/// titles are formatted "Security: {dep} \u{2014} {source title}" - the prefix
/// contains the injected dep label and must not count as evidence, so only the
/// substring after the first em-dash is examined. Returns the deleted count.
pub(crate) fn revalidate_recorded_wins(conn: &Connection) -> i64 {
    let rows: Vec<(i64, String, String)> = {
        let mut stmt = match conn.prepare(
            "SELECT id, COALESCE(affected_deps, ''), COALESCE(alert_title, '') \
             FROM preemption_wins WHERE verified = 1",
        ) {
            Ok(s) => s,
            Err(e) => {
                warn!(target: "4da::decision_advantage", error = %e, "Win revalidation query failed");
                return 0;
            }
        };
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .ok()
            .map(|rows| rows.filter_map(std::result::Result::ok).collect())
            .unwrap_or_default()
    };

    let mut deleted = 0i64;
    for (id, deps, title) in rows {
        let dep = deps.trim().to_lowercase();
        if dep.is_empty() || !crate::package_ambiguity::requires_strict_proof(&dep) {
            continue;
        }
        let source_title = match title.split_once('\u{2014}') {
            Some((_, rest)) => rest,
            None => title.as_str(),
        };
        let source_title_lower = source_title.to_lowercase();
        if crate::package_ambiguity::has_word_boundary_match(&source_title_lower, &dep) {
            continue;
        }
        match conn.execute("DELETE FROM preemption_wins WHERE id = ?1", params![id]) {
            Ok(_) => {
                deleted += 1;
                warn!(
                    target: "4da::decision_advantage",
                    id, dep = %dep, title = %title,
                    "Deleted false preemption win (dep not grounded in source title)"
                );
            }
            Err(e) => {
                warn!(
                    target: "4da::decision_advantage",
                    id, error = %e,
                    "Failed to delete false preemption win"
                );
            }
        }
    }
    if deleted > 0 {
        info!(
            target: "4da::decision_advantage",
            deleted,
            "Win revalidation purged false preemption wins"
        );
    }
    deleted
}

/// Lead time in hours between window open and incident. None if either timestamp
/// fails to parse with the canonical "%Y-%m-%d %H:%M:%S" format windows.rs uses.
fn compute_lead_time(opened_at: &str, incident_at: &str) -> Option<f32> {
    let opened = chrono::NaiveDateTime::parse_from_str(opened_at, "%Y-%m-%d %H:%M:%S").ok()?;
    let incident = chrono::NaiveDateTime::parse_from_str(incident_at, "%Y-%m-%d %H:%M:%S").ok()?;
    Some((incident - opened).num_minutes() as f32 / 60.0)
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;

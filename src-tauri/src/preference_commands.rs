// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Learned Preferences commands — exposes the stability detector's
//! preference lifecycle to the frontend (P6: Compound Lattice).

use crate::error::Result;
use crate::stability_detector;

/// Returns learned facets suitable for the preferences UI.
///
/// Only surfaces facets a human would recognize: user overrides, languages,
/// frameworks, sources, vetoes.  Topic affinities are internal scoring
/// signals — never shown.  Hard-capped at 25 items.
#[tauri::command]
pub async fn get_learned_preferences() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let facets = stability_detector::get_all_learned_facets(&conn);
    let disabled = disabled_source_types(&conn);

    let mut items: Vec<serde_json::Value> = facets
        .iter()
        .filter(|f| is_listed(f, &disabled))
        .map(|f| {
            serde_json::json!({
                "facet_id": f.facet_id,
                "class": f.class.as_str(),
                "key": f.key,
                "value": f.value,
                "stability": (f.stability * 100.0).round() / 100.0,
                "state": f.state.as_str(),
                "user_state": f.user_state.as_str(),
                "evidence_count": f.evidence_count,
                "first_seen_at": f.first_seen_at,
                "last_seen_at": f.last_seen_at,
            })
        })
        .collect();

    items.truncate(25);

    Ok(serde_json::json!({
        "facets": items,
        "count": items.len(),
    }))
}

/// Source types the user turned off.
fn disabled_source_types(conn: &rusqlite::Connection) -> Vec<String> {
    conn.prepare("SELECT source_type FROM sources WHERE enabled = 0")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<String>>>()
        })
        .unwrap_or_default()
}

/// Whether the preferences UI lists a facet. User overrides always; topic
/// affinities never (internal scoring signals); otherwise active or
/// provisional facets — except a learned preference for a source that is
/// turned off. Audit L10 (2026-10-12): "RSS Feeds / Dev.to · high" were
/// listed while both sources were disabled — a preference nothing can act on.
/// The facet itself is kept, so it applies again if the source is turned on.
fn is_listed(f: &stability_detector::LearnedFacet, disabled_sources: &[String]) -> bool {
    if f.user_state != stability_detector::UserState::Auto {
        return true;
    }
    if f.class == stability_detector::FacetClass::TopicAffinity {
        return false;
    }
    if f.class == stability_detector::FacetClass::SourcePref
        && disabled_sources.iter().any(|s| s == &f.key)
    {
        return false;
    }
    matches!(
        f.state,
        stability_detector::FacetState::Active | stability_detector::FacetState::Provisional
    )
}

/// Pin a facet — user explicitly confirms this preference.
#[tauri::command]
pub async fn pin_preference(facet_id: String) -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let ok =
        stability_detector::set_user_state(&conn, &facet_id, stability_detector::UserState::Pinned);
    if ok {
        tracing::info!(target: "4da::preferences", facet_id = %facet_id, "Preference pinned");
        Ok(serde_json::json!({ "success": true }))
    } else {
        Err(format!("Facet '{}' not found", facet_id).into())
    }
}

/// Forget a facet — user explicitly dismisses this preference.
#[tauri::command]
pub async fn forget_preference(facet_id: String) -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let ok = stability_detector::set_user_state(
        &conn,
        &facet_id,
        stability_detector::UserState::Forgotten,
    );
    if ok {
        tracing::info!(target: "4da::preferences", facet_id = %facet_id, "Preference forgotten");
        Ok(serde_json::json!({ "success": true }))
    } else {
        Err(format!("Facet '{}' not found", facet_id).into())
    }
}

/// Reset a facet back to automatic control (undo pin/forget).
#[tauri::command]
pub async fn reset_preference(facet_id: String) -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let ok =
        stability_detector::set_user_state(&conn, &facet_id, stability_detector::UserState::Auto);
    if ok {
        tracing::info!(target: "4da::preferences", facet_id = %facet_id, "Preference reset to auto");
        Ok(serde_json::json!({ "success": true }))
    } else {
        Err(format!("Facet '{}' not found", facet_id).into())
    }
}

/// Get evidence trail for a specific facet.
#[tauri::command]
pub async fn get_preference_evidence(facet_id: String) -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let evidence = stability_detector::get_facet_evidence(&conn, &facet_id);

    let items: Vec<serde_json::Value> = evidence
        .iter()
        .map(|e| {
            serde_json::json!({
                "cue_family": e.cue_family.as_str(),
                "evidence_type": e.evidence_type,
                "confidence": e.confidence,
                "observed_at": e.observed_at,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "facet_id": facet_id,
        "evidence": items,
        "count": items.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stability_detector::{FacetClass, FacetState, LearnedFacet, UserState};

    fn facet(class: FacetClass, key: &str, user_state: UserState) -> LearnedFacet {
        LearnedFacet {
            facet_id: format!("{}:{key}", class.as_str()),
            class,
            key: key.to_string(),
            value: "high".to_string(),
            stability: 1.5,
            state: FacetState::Active,
            user_state,
            evidence_count: 15,
            first_seen_at: 0,
            last_seen_at: 0,
        }
    }

    /// Audit L10: a learned preference for a turned-off source is not listed;
    /// one for an enabled source, or one the user pinned, still is.
    #[test]
    fn a_source_preference_for_a_disabled_source_is_not_listed() {
        let disabled = vec!["rss".to_string(), "devto".to_string()];
        assert!(!is_listed(
            &facet(FacetClass::SourcePref, "rss", UserState::Auto),
            &disabled
        ));
        assert!(!is_listed(
            &facet(FacetClass::SourcePref, "devto", UserState::Auto),
            &disabled
        ));
        assert!(is_listed(
            &facet(FacetClass::SourcePref, "crates_io", UserState::Auto),
            &disabled
        ));
        assert!(is_listed(
            &facet(FacetClass::SourcePref, "rss", UserState::Pinned),
            &disabled
        ));
        // Only source preferences are matched by source name.
        assert!(is_listed(
            &facet(FacetClass::Interest, "rss", UserState::Auto),
            &disabled
        ));
    }

    #[test]
    fn disabled_source_types_reads_the_sources_table() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sources (source_type TEXT PRIMARY KEY, enabled INTEGER NOT NULL);
             INSERT INTO sources VALUES ('rss', 0), ('crates_io', 1), ('devto', 0);",
        )
        .unwrap();
        let mut off = disabled_source_types(&conn);
        off.sort();
        assert_eq!(off, vec!["devto".to_string(), "rss".to_string()]);
    }
}

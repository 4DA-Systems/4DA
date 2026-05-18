// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Learned Preferences commands — exposes the stability detector's
//! preference lifecycle to the frontend (P6: Compound Lattice).

use crate::error::Result;
use crate::stability_detector;

/// Returns all learned facets grouped for the preferences UI.
#[tauri::command]
pub async fn get_learned_preferences() -> Result<serde_json::Value> {
    let conn = crate::open_db_connection()?;
    let facets = stability_detector::get_all_learned_facets(&conn);

    let items: Vec<serde_json::Value> = facets
        .iter()
        .filter(|f| {
            // Never show forgotten facets in the main interests view.
            // Users can see them in a separate "filtered" section if needed.
            if f.user_state == stability_detector::UserState::Forgotten {
                return false;
            }
            // Include if: user pinned, or has real evidence, or active/provisional
            f.user_state == stability_detector::UserState::Pinned
                || f.evidence_count > 1
                || matches!(
                    f.state,
                    stability_detector::FacetState::Active
                        | stability_detector::FacetState::Provisional
                )
        })
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

    Ok(serde_json::json!({
        "facets": items,
        "count": items.len(),
    }))
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

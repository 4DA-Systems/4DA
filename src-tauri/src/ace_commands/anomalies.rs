// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! ACE anomaly detection commands.

use crate::error::Result;

/// Get all unresolved anomalies
#[tauri::command]
pub async fn ace_get_unresolved_anomalies() -> Result<serde_json::Value> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = crate::anomaly::get_unresolved(&conn)?;
    Ok(serde_json::json!({
        "anomalies": anomalies,
        "count": anomalies.len()
    }))
}

/// Run anomaly detection and store results
#[tauri::command]
pub async fn ace_detect_anomalies() -> Result<serde_json::Value> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    let anomalies = crate::anomaly::detect_all(&conn)?;
    for a in &anomalies {
        if let Err(e) = crate::anomaly::store_anomaly(&conn, a) {
            tracing::warn!("Failed to store anomaly: {e}");
        }
    }
    Ok(serde_json::json!({
        "anomalies": anomalies,
        "count": anomalies.len()
    }))
}

/// Resolve (dismiss) an anomaly by id
#[tauri::command]
pub async fn ace_resolve_anomaly(anomaly_id: i64) -> Result<()> {
    let ace = crate::get_ace_engine()?;
    let conn = ace.get_conn().lock();
    crate::anomaly::resolve_anomaly(&conn, anomaly_id)
}

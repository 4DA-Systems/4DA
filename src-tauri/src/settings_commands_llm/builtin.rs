// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Built-in LLM model management: sidecar lifecycle, catalog, download, delete.

use crate::error::Result;
use tauri::{AppHandle, Emitter};

/// Implementation for start_builtin_llm command.
pub(super) async fn start_builtin_llm_impl(model_id: String) -> Result<serde_json::Value> {
    let entry = crate::model_manager::find_model(&model_id)
        .ok_or_else(|| crate::error::FourDaError::Llm(format!("Unknown model: {model_id}")))?;
    let path = crate::model_manager::model_path(entry).ok_or_else(|| {
        crate::error::FourDaError::Llm(format!(
            "Model {} is not downloaded yet",
            entry.display_name
        ))
    })?;
    let port = crate::llm_engine::start_sidecar(&path).await?;
    Ok(serde_json::json!({
        "status": "ready",
        "port": port,
        "model_id": model_id,
    }))
}

/// Implementation for stop_builtin_llm command.
pub(super) fn stop_builtin_llm_impl() -> Result<serde_json::Value> {
    crate::llm_engine::stop_sidecar();
    Ok(serde_json::json!({ "status": "stopped" }))
}

/// Implementation for get_builtin_llm_status command.
pub(super) fn get_builtin_llm_status_impl() -> Result<serde_json::Value> {
    let status = crate::llm_engine::sidecar_status();
    let port = crate::llm_engine::sidecar_port();
    Ok(serde_json::json!({
        "status": status,
        "port": port,
    }))
}

/// Implementation for list_builtin_models command.
pub(super) fn list_builtin_models_impl() -> Result<serde_json::Value> {
    let hw = crate::hardware_detect::detect_hardware();
    let catalog: Vec<serde_json::Value> = crate::model_manager::model_catalog()
        .iter()
        .map(|entry| {
            let downloaded = crate::model_manager::is_model_downloaded(entry);
            let path = crate::model_manager::model_path(entry);
            serde_json::json!({
                "id": entry.id,
                "display_name": entry.display_name,
                "family": entry.family,
                "size_bytes": entry.size_bytes,
                "size_gb": (entry.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0) * 10.0).round() / 10.0,
                "min_ram_gb": entry.min_ram_gb,
                "quantization": entry.quantization,
                "downloaded": downloaded,
                "path": path,
                "fits_ram": entry.min_ram_gb <= hw.ram_total_gb,
            })
        })
        .collect();

    let recommended = crate::model_manager::recommend_model(hw.ram_available_gb);

    Ok(serde_json::json!({
        "models": catalog,
        "recommended_id": recommended.map(|e| e.id),
        "ram_total_gb": hw.ram_total_gb,
        "ram_available_gb": hw.ram_available_gb,
    }))
}

/// Implementation for download_builtin_model command.
pub(super) async fn download_builtin_model_impl(
    app_handle: AppHandle,
    model_id: String,
) -> Result<serde_json::Value> {
    let entry = crate::model_manager::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;

    let handle = app_handle.clone();
    let path = crate::model_manager::download_model(entry, move |progress| {
        let _ = handle.emit("model-download-progress", &progress);
    })
    .await?;

    Ok(serde_json::json!({
        "model_id": model_id,
        "path": path,
        "status": "complete",
    }))
}

/// Implementation for cancel_builtin_model_download command.
pub(super) fn cancel_builtin_model_download_impl() -> Result<String> {
    crate::model_manager::cancel_download();
    Ok("Download cancellation requested".into())
}

/// Implementation for delete_builtin_model command.
pub(super) fn delete_builtin_model_impl(model_id: String) -> Result<serde_json::Value> {
    let entry = crate::model_manager::find_model(&model_id)
        .ok_or_else(|| format!("Unknown model: {model_id}"))?;
    crate::model_manager::delete_model(entry)?;
    Ok(serde_json::json!({
        "model_id": model_id,
        "status": "deleted",
    }))
}

//! Job queue Tauri commands for background extraction processing.
//!
//! Extracted from lib.rs. Provides commands for creating, querying,
//! and managing background file extraction jobs.

use crate::get_job_queue;
use crate::job_queue;

// ============================================================================
// Job Queue Commands (Background Extraction Processing)
// ============================================================================

/// Create an extraction job for a file
#[tauri::command]
pub async fn create_extraction_job(file_path: String, file_type: String) -> Result<i64, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.create_job(&file_path, &file_type)
}

/// Get a specific extraction job
#[tauri::command]
pub async fn get_extraction_job(job_id: i64) -> Result<Option<job_queue::ExtractionJob>, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.get_job(job_id)
}

/// Get extraction jobs with optional status filter
#[tauri::command]
pub async fn get_extraction_jobs(
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<job_queue::ExtractionJob>, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    let status = status.map(|s| job_queue::JobStatus::from_str(&s));
    queue.get_jobs(status, limit.unwrap_or(50))
}

/// Get job queue statistics
#[tauri::command]
pub async fn get_job_queue_stats() -> Result<job_queue::QueueStats, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.get_stats()
}

/// Cancel an extraction job
#[tauri::command]
pub async fn cancel_extraction_job(job_id: i64) -> Result<(), String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.cancel_job(job_id)
}

/// Start the job queue background worker
#[tauri::command]
pub async fn start_job_queue_worker() -> Result<(), String> {
    let queue = get_job_queue()?;
    let mut queue = queue.write();
    queue.start_worker()
}

/// Stop the job queue background worker
#[tauri::command]
pub async fn stop_job_queue_worker() -> Result<(), String> {
    let queue = get_job_queue()?;
    let mut queue = queue.write();
    queue.stop_worker();
    Ok(())
}

/// Clean up old completed/failed jobs
#[tauri::command]
pub async fn cleanup_extraction_jobs(days: Option<u32>) -> Result<usize, String> {
    let queue = get_job_queue()?;
    let queue = queue.read();
    queue.cleanup_old_jobs(days.unwrap_or(7))
}

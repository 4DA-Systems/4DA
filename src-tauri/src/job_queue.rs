#![allow(dead_code)]
//! Background Job Queue for Document Extraction
//!
//! Handles async processing of document extraction jobs (OCR, audio transcription).
//! Uses the extraction_jobs table for persistence and recovery.

use crate::extractors::{ExtractedDocument, ExtractorRegistry};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Processing => "processing",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => JobStatus::Pending,
            "processing" => JobStatus::Processing,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            _ => JobStatus::Pending,
        }
    }
}

/// An extraction job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionJob {
    pub id: i64,
    pub file_path: String,
    pub file_type: String,
    pub status: JobStatus,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub extracted_chunks: i32,
    pub created_at: String,
}

/// Job queue manager
pub struct JobQueue {
    conn: Arc<Mutex<Connection>>,
    running: Arc<AtomicBool>,
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl JobQueue {
    /// Create a new job queue
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self {
            conn,
            running: Arc::new(AtomicBool::new(false)),
            worker_handle: None,
        }
    }

    /// Create a new extraction job
    pub fn create_job(&self, file_path: &str, file_type: &str) -> Result<i64, String> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO extraction_jobs (file_path, file_type, status) VALUES (?1, ?2, 'pending')",
            [file_path, file_type],
        )
        .map_err(|e| format!("Failed to create job: {}", e))?;

        let job_id = conn.last_insert_rowid();
        info!(target: "job_queue", job_id = job_id, file_path = file_path, "Created extraction job");
        Ok(job_id)
    }

    /// Get a job by ID
    pub fn get_job(&self, job_id: i64) -> Result<Option<ExtractionJob>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, file_path, file_type, status, error, started_at, completed_at,
                        extracted_chunks, created_at
                 FROM extraction_jobs WHERE id = ?1",
            )
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let job = stmt
            .query_row([job_id], |row| {
                Ok(ExtractionJob {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_type: row.get(2)?,
                    status: JobStatus::from_str(&row.get::<_, String>(3)?),
                    error: row.get(4)?,
                    started_at: row.get(5)?,
                    completed_at: row.get(6)?,
                    extracted_chunks: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .optional()
            .map_err(|e| format!("Failed to get job: {}", e))?;

        Ok(job)
    }

    /// Get all jobs with optional status filter
    pub fn get_jobs(
        &self,
        status: Option<JobStatus>,
        limit: usize,
    ) -> Result<Vec<ExtractionJob>, String> {
        let conn = self.conn.lock();

        let (sql, params): (&str, Vec<String>) = match status {
            Some(s) => (
                "SELECT id, file_path, file_type, status, error, started_at, completed_at,
                        extracted_chunks, created_at
                 FROM extraction_jobs WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
                vec![s.as_str().to_string(), limit.to_string()],
            ),
            None => (
                "SELECT id, file_path, file_type, status, error, started_at, completed_at,
                        extracted_chunks, created_at
                 FROM extraction_jobs ORDER BY created_at DESC LIMIT ?1",
                vec![limit.to_string()],
            ),
        };

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let jobs = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(ExtractionJob {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_type: row.get(2)?,
                    status: JobStatus::from_str(&row.get::<_, String>(3)?),
                    error: row.get(4)?,
                    started_at: row.get(5)?,
                    completed_at: row.get(6)?,
                    extracted_chunks: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("Failed to query jobs: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(jobs)
    }

    /// Get the next pending job
    fn get_next_pending(&self) -> Result<Option<ExtractionJob>, String> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, file_path, file_type, status, error, started_at, completed_at,
                        extracted_chunks, created_at
                 FROM extraction_jobs WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1",
            )
            .map_err(|e| format!("Failed to prepare statement: {}", e))?;

        let job = stmt
            .query_row([], |row| {
                Ok(ExtractionJob {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_type: row.get(2)?,
                    status: JobStatus::from_str(&row.get::<_, String>(3)?),
                    error: row.get(4)?,
                    started_at: row.get(5)?,
                    completed_at: row.get(6)?,
                    extracted_chunks: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .optional()
            .map_err(|e| format!("Failed to get pending job: {}", e))?;

        Ok(job)
    }

    /// Update job status
    fn update_status(
        &self,
        job_id: i64,
        status: JobStatus,
        error: Option<&str>,
        chunks: Option<i32>,
    ) -> Result<(), String> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();

        match status {
            JobStatus::Processing => {
                conn.execute(
                    "UPDATE extraction_jobs SET status = ?1, started_at = ?2 WHERE id = ?3",
                    rusqlite::params![status.as_str(), now, job_id],
                )
                .map_err(|e| format!("Failed to update job: {}", e))?;
            }
            JobStatus::Completed => {
                conn.execute(
                    "UPDATE extraction_jobs SET status = ?1, completed_at = ?2, extracted_chunks = ?3 WHERE id = ?4",
                    rusqlite::params![status.as_str(), now, chunks.unwrap_or(0), job_id],
                )
                .map_err(|e| format!("Failed to update job: {}", e))?;
            }
            JobStatus::Failed => {
                conn.execute(
                    "UPDATE extraction_jobs SET status = ?1, completed_at = ?2, error = ?3 WHERE id = ?4",
                    rusqlite::params![status.as_str(), now, error, job_id],
                )
                .map_err(|e| format!("Failed to update job: {}", e))?;
            }
            JobStatus::Pending => {
                conn.execute(
                    "UPDATE extraction_jobs SET status = 'pending' WHERE id = ?1",
                    [job_id],
                )
                .map_err(|e| format!("Failed to update job: {}", e))?;
            }
        }

        debug!(target: "job_queue", job_id = job_id, status = ?status, "Updated job status");
        Ok(())
    }

    /// Cancel a job
    pub fn cancel_job(&self, job_id: i64) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE extraction_jobs SET status = 'failed', error = 'Cancelled by user' WHERE id = ?1 AND status IN ('pending', 'processing')",
            [job_id],
        )
        .map_err(|e| format!("Failed to cancel job: {}", e))?;

        info!(target: "job_queue", job_id = job_id, "Cancelled job");
        Ok(())
    }

    /// Delete completed/failed jobs older than N days
    pub fn cleanup_old_jobs(&self, days: u32) -> Result<usize, String> {
        let conn = self.conn.lock();
        let deleted = conn
            .execute(
                "DELETE FROM extraction_jobs
                 WHERE status IN ('completed', 'failed')
                 AND created_at < datetime('now', ?1)",
                [format!("-{} days", days)],
            )
            .map_err(|e| format!("Failed to cleanup jobs: {}", e))?;

        info!(target: "job_queue", deleted = deleted, "Cleaned up old jobs");
        Ok(deleted)
    }

    /// Process a single job
    fn process_job(&self, job: &ExtractionJob) -> Result<ExtractedDocument, String> {
        let path = PathBuf::from(&job.file_path);

        if !path.exists() {
            return Err(format!("File not found: {}", job.file_path));
        }

        let registry = ExtractorRegistry::new();
        registry.extract(&path)
    }

    /// Start the background worker
    pub fn start_worker(&mut self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let conn = self.conn.clone();

        let handle = thread::spawn(move || {
            info!(target: "job_queue", "Background worker started");

            while running.load(Ordering::SeqCst) {
                // Create a temporary queue to process jobs
                let queue = JobQueue {
                    conn: conn.clone(),
                    running: running.clone(),
                    worker_handle: None,
                };

                // Get next pending job
                match queue.get_next_pending() {
                    Ok(Some(job)) => {
                        info!(target: "job_queue", job_id = job.id, file = %job.file_path, "Processing job");

                        // Mark as processing
                        if let Err(e) =
                            queue.update_status(job.id, JobStatus::Processing, None, None)
                        {
                            error!(target: "job_queue", error = %e, "Failed to update job status");
                            continue;
                        }

                        // Process the job
                        match queue.process_job(&job) {
                            Ok(doc) => {
                                let chunks = doc.pages.len().max(1) as i32;
                                if let Err(e) = queue.update_status(
                                    job.id,
                                    JobStatus::Completed,
                                    None,
                                    Some(chunks),
                                ) {
                                    error!(target: "job_queue", error = %e, "Failed to mark job complete");
                                }
                                info!(target: "job_queue", job_id = job.id, chunks = chunks, "Job completed");
                            }
                            Err(e) => {
                                warn!(target: "job_queue", job_id = job.id, error = %e, "Job failed");
                                if let Err(e2) =
                                    queue.update_status(job.id, JobStatus::Failed, Some(&e), None)
                                {
                                    error!(target: "job_queue", error = %e2, "Failed to mark job failed");
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // No pending jobs, sleep
                        thread::sleep(Duration::from_secs(1));
                    }
                    Err(e) => {
                        error!(target: "job_queue", error = %e, "Failed to get pending job");
                        thread::sleep(Duration::from_secs(5));
                    }
                }
            }

            info!(target: "job_queue", "Background worker stopped");
        });

        self.worker_handle = Some(handle);
        Ok(())
    }

    /// Stop the background worker
    pub fn stop_worker(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        info!(target: "job_queue", "Worker stopped");
    }

    /// Check if worker is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get queue statistics
    pub fn get_stats(&self) -> Result<QueueStats, String> {
        let conn = self.conn.lock();

        let pending: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to get pending count: {}", e))?;

        let processing: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'processing'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to get processing count: {}", e))?;

        let completed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'completed'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to get completed count: {}", e))?;

        let failed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM extraction_jobs WHERE status = 'failed'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to get failed count: {}", e))?;

        Ok(QueueStats {
            pending: pending as u32,
            processing: processing as u32,
            completed: completed as u32,
            failed: failed as u32,
            is_running: self.is_running(),
        })
    }
}

impl Drop for JobQueue {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    pub pending: u32,
    pub processing: u32,
    pub completed: u32,
    pub failed: u32,
    pub is_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE extraction_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'failed')),
                error TEXT,
                started_at TEXT,
                completed_at TEXT,
                extracted_chunks INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        ).unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_create_and_get_job() {
        let conn = setup_test_db();
        let queue = JobQueue::new(conn);

        let job_id = queue.create_job("/test/file.pdf", "pdf").unwrap();
        assert!(job_id > 0);

        let job = queue.get_job(job_id).unwrap().unwrap();
        assert_eq!(job.file_path, "/test/file.pdf");
        assert_eq!(job.file_type, "pdf");
        assert_eq!(job.status, JobStatus::Pending);
    }

    #[test]
    fn test_job_status_transitions() {
        let conn = setup_test_db();
        let queue = JobQueue::new(conn);

        let job_id = queue.create_job("/test/file.png", "image").unwrap();

        // Mark as processing
        queue
            .update_status(job_id, JobStatus::Processing, None, None)
            .unwrap();
        let job = queue.get_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Processing);
        assert!(job.started_at.is_some());

        // Mark as completed
        queue
            .update_status(job_id, JobStatus::Completed, None, Some(5))
            .unwrap();
        let job = queue.get_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.extracted_chunks, 5);
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn test_job_failure() {
        let conn = setup_test_db();
        let queue = JobQueue::new(conn);

        let job_id = queue.create_job("/test/file.wav", "audio").unwrap();

        queue
            .update_status(
                job_id,
                JobStatus::Failed,
                Some("OCR engine not available"),
                None,
            )
            .unwrap();
        let job = queue.get_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error, Some("OCR engine not available".to_string()));
    }

    #[test]
    fn test_get_stats() {
        let conn = setup_test_db();
        let queue = JobQueue::new(conn);

        queue.create_job("/test/1.pdf", "pdf").unwrap();
        queue.create_job("/test/2.pdf", "pdf").unwrap();
        let job3 = queue.create_job("/test/3.pdf", "pdf").unwrap();
        queue
            .update_status(job3, JobStatus::Completed, None, Some(1))
            .unwrap();

        let stats = queue.get_stats().unwrap();
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.completed, 1);
    }

    #[test]
    fn test_cancel_job() {
        let conn = setup_test_db();
        let queue = JobQueue::new(conn);

        let job_id = queue.create_job("/test/file.pdf", "pdf").unwrap();
        queue.cancel_job(job_id).unwrap();

        let job = queue.get_job(job_id).unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Failed);
        assert!(job.error.unwrap().contains("Cancelled"));
    }
}

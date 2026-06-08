// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Engine-run freshness receipts.
//!
//! Every fetch+score cycle — whether driven by the GUI's background scheduler or by the
//! headless `fourda-engine` binary — writes a row here recording *what actually happened*.
//! This is the ground-truth layer the MCP server reads to stop serving stale data as if it
//! were live, and the state an external verifier (Verax) re-checks: a refresh that exits 0
//! but silently no-ops is divergence, so the receipt records the real watermark and a content
//! fingerprint, not just "the cycle ran".
//!
//! The table is self-created via `CREATE TABLE IF NOT EXISTS` (no schema migration) so it
//! materializes on first cycle in both the GUI app and the headless binary, against the same
//! `data/4da.db`.

use anyhow::Result;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

// ============================================================================
// Schema (self-created, no migration)
// ============================================================================

pub(crate) const ENGINE_RUNS_SQL: &str = "
CREATE TABLE IF NOT EXISTS engine_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    sources_succeeded INTEGER NOT NULL DEFAULT 0,
    sources_failed INTEGER NOT NULL DEFAULT 0,
    sources_skipped INTEGER NOT NULL DEFAULT 0,
    new_items INTEGER NOT NULL DEFAULT 0,
    cached_touches INTEGER NOT NULL DEFAULT 0,
    items_scored INTEGER NOT NULL DEFAULT 0,
    relevant_count INTEGER NOT NULL DEFAULT 0,
    source_items_total INTEGER NOT NULL DEFAULT 0,
    max_item_created_at TEXT,
    content_fingerprint TEXT NOT NULL DEFAULT '',
    ok INTEGER NOT NULL DEFAULT 1,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_engine_runs_completed ON engine_runs(completed_at);
";

/// Create the `engine_runs` table if it does not exist. Idempotent.
pub(crate) fn ensure_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(ENGINE_RUNS_SQL)
}

// ============================================================================
// Receipt — what a cycle did
// ============================================================================

/// What one fetch+score cycle produced. Watermark/fingerprint/totals are derived from the live
/// DB at `record()` time, so the caller only supplies what it directly measured.
#[derive(Debug, Clone)]
pub(crate) struct RunReceipt {
    /// Origin of the cycle: `scheduled` (GUI background), `foreground` (user-triggered),
    /// `headless_once`, or `headless_daemon`.
    pub trigger: &'static str,
    pub started_at: String,
    pub completed_at: String,
    pub duration_ms: u64,
    pub sources_succeeded: usize,
    pub sources_failed: usize,
    pub sources_skipped: usize,
    pub new_items: usize,
    pub cached_touches: usize,
    pub items_scored: usize,
    pub relevant_count: usize,
    pub ok: bool,
    pub error: Option<String>,
}

impl RunReceipt {
    /// Start a receipt for a cycle of the given trigger, stamping `started_at` now.
    pub fn begin(trigger: &'static str) -> Self {
        Self {
            trigger,
            started_at: now_rfc3339(),
            completed_at: String::new(),
            duration_ms: 0,
            sources_succeeded: 0,
            sources_failed: 0,
            sources_skipped: 0,
            new_items: 0,
            cached_touches: 0,
            items_scored: 0,
            relevant_count: 0,
            ok: true,
            error: None,
        }
    }
}

/// Persist a cycle receipt. Best-effort: a failure here must never break the cycle, so errors are
/// logged and swallowed. Derives the freshness watermark, source-item total, and content
/// fingerprint from the live DB so the row reflects ground truth rather than the caller's claim.
pub(crate) fn record(mut receipt: RunReceipt) {
    if receipt.completed_at.is_empty() {
        receipt.completed_at = now_rfc3339();
    }
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::engine_runs", error = %e, "Skipping receipt — database unavailable");
            return;
        }
    };
    let conn = db.conn.lock();
    if let Err(e) = ensure_table(&conn) {
        warn!(target: "4da::engine_runs", error = %e, "Skipping receipt — ensure_table failed");
        return;
    }

    let (total, watermark) = source_items_state(&conn);
    let fingerprint = fingerprint(total, watermark.as_deref());

    let res = conn.execute(
        "INSERT INTO engine_runs (
            trigger, started_at, completed_at, duration_ms,
            sources_succeeded, sources_failed, sources_skipped,
            new_items, cached_touches, items_scored, relevant_count,
            source_items_total, max_item_created_at, content_fingerprint, ok, error
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        rusqlite::params![
            receipt.trigger,
            receipt.started_at,
            receipt.completed_at,
            receipt.duration_ms as i64,
            receipt.sources_succeeded as i64,
            receipt.sources_failed as i64,
            receipt.sources_skipped as i64,
            receipt.new_items as i64,
            receipt.cached_touches as i64,
            receipt.items_scored as i64,
            receipt.relevant_count as i64,
            total,
            watermark,
            fingerprint,
            receipt.ok as i64,
            receipt.error,
        ],
    );
    match res {
        Ok(_) => debug!(
            target: "4da::engine_runs",
            trigger = receipt.trigger,
            new_items = receipt.new_items,
            scored = receipt.items_scored,
            total,
            ok = receipt.ok,
            "Engine-run receipt recorded"
        ),
        Err(e) => {
            warn!(target: "4da::engine_runs", error = %e, "Failed to insert engine-run receipt")
        }
    }
}

// ============================================================================
// Freshness snapshot — ground truth for the MCP server and external verifiers
// ============================================================================

/// The live freshness state of the database, read directly from the source tables (not the
/// receipt). This is the contract an external verifier asserts against: "is the data actually
/// fresh", answered from `source_items` and `sources`, independent of any claim that a refresh ran.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct FreshnessSnapshot {
    /// Total rows in `source_items`.
    pub source_items_total: i64,
    /// Newest `source_items.created_at` (the ingestion watermark), or None if empty.
    pub max_item_created_at: Option<String>,
    /// Newest `sources.last_fetch` across all sources, or None if nothing has fetched.
    pub last_source_fetch: Option<String>,
    /// `completed_at` of the most recent engine run, or None if no cycle has run.
    pub last_run_completed_at: Option<String>,
    /// Whether the most recent engine run reported success.
    pub last_run_ok: Option<bool>,
    /// `trigger` of the most recent engine run.
    pub last_run_trigger: Option<String>,
    /// Deterministic fingerprint of (total, watermark) — changes iff content moved.
    pub content_fingerprint: String,
}

/// Read the live freshness snapshot from the database. Reads ground truth; does not trust receipts.
pub(crate) fn freshness_snapshot() -> Result<FreshnessSnapshot> {
    let db = crate::get_database()?;
    let conn = db.conn.lock();
    // The engine_runs table may not exist yet on a brand-new DB — ensure it so the read can't fail.
    let _ = ensure_table(&conn);

    let (source_items_total, max_item_created_at) = source_items_state(&conn);

    let last_source_fetch: Option<String> = conn
        .query_row("SELECT MAX(last_fetch) FROM sources", [], |r| r.get(0))
        .unwrap_or(None);

    let (last_run_completed_at, last_run_ok, last_run_trigger): (
        Option<String>,
        Option<bool>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT completed_at, ok, trigger FROM engine_runs ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<i64>>(1)?.map(|v| v != 0),
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .unwrap_or((None, None, None));

    Ok(FreshnessSnapshot {
        source_items_total,
        content_fingerprint: fingerprint(source_items_total, max_item_created_at.as_deref()),
        max_item_created_at,
        last_source_fetch,
        last_run_completed_at,
        last_run_ok,
        last_run_trigger,
    })
}

// ============================================================================
// Helpers
// ============================================================================

/// `(COUNT(*), MAX(created_at))` of `source_items`. Returns `(0, None)` on any error so callers
/// never have to handle the failure path for a best-effort freshness read.
fn source_items_state(conn: &Connection) -> (i64, Option<String>) {
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM source_items", [], |r| r.get(0))
        .unwrap_or(0);
    let watermark: Option<String> = conn
        .query_row("SELECT MAX(created_at) FROM source_items", [], |r| r.get(0))
        .unwrap_or(None);
    (total, watermark)
}

/// Deterministic content fingerprint over (total, watermark). Changes iff row count or the newest
/// item timestamp changes — i.e. iff content actually moved. Lets a verifier prove "data changed"
/// without diffing the whole table.
fn fingerprint(total: i64, watermark: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(total.to_le_bytes());
    hasher.update(b":");
    hasher.update(watermark.unwrap_or("").as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..16]) // 128-bit prefix is ample for change-detection
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

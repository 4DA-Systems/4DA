// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Engine-run freshness receipts.
//!
//! Every fetch+score cycle — whether driven by the GUI's background scheduler or by the
//! headless `fourda-engine` binary — writes a row here recording *what actually happened*.
//! This is the ground-truth layer the MCP server reads to stop serving stale data as if it
//! were live, and the state an external verifier re-checks: a refresh that exits 0
//! but silently no-ops is divergence, so the receipt records the real watermark and a content
//! fingerprint, not just "the cycle ran".
//!
//! The table is self-created via `CREATE TABLE IF NOT EXISTS` (no schema migration) so it
//! materializes on first cycle in both the GUI app and the headless binary, against the same
//! `data/4da.db`.

use anyhow::Result;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use once_cell::sync::OnceCell;
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
    nonce TEXT,
    signature TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_engine_runs_completed ON engine_runs(completed_at);
";

/// Create the `engine_runs` table if it does not exist. Idempotent.
pub(crate) fn ensure_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(ENGINE_RUNS_SQL)?;
    // Idempotent adds for databases created before these columns existed (duplicate-column errors
    // are intentionally ignored). `nonce` binds a receipt to a verifier-assigned task (defeats a
    // free-ride on a concurrent refresh). `signature` is an Ed25519 signature over the receipt's
    // canonical fields, with the private key held in the OS keychain the agent can't reach — so a
    // forged/cloned row has no valid signature and an attribution proof can diverge it.
    let _ = conn.execute("ALTER TABLE engine_runs ADD COLUMN nonce TEXT", []);
    let _ = conn.execute("ALTER TABLE engine_runs ADD COLUMN signature TEXT", []);
    Ok(())
}

// ============================================================================
// Receipt signing (Ed25519) — un-forgeable attribution
// ============================================================================
//
// Each receipt is signed with an Ed25519 key whose private half lives in the OS keychain (via
// `settings::keystore`), unreachable by a same-user DB-write adversary. The public half is published
// to `<data_dir>/engine_receipt_pubkey.hex` so an external verifier can read it ONCE and pin it. A
// proof then verifies the signature over the row's canonical fields: a row that was cloned or
// `INSERT`ed by a forger carries no valid signature → the proof diverges it. Signing is best-effort
// — if the keychain/CSPRNG is unavailable the receipt is recorded unsigned rather than the cycle failing.

/// Keychain entry name for the receipt signing key (32-byte Ed25519 seed, hex-encoded).
const RECEIPT_SIGNING_KEY: &str = "engine_receipt_signing_key";
/// Process-cached signing-key seed (loaded/generated once); `None` if signing is unavailable.
static SIGNING_SEED: OnceCell<Option<[u8; 32]>> = OnceCell::new();

/// Load the receipt signing-key seed from the keychain, generating + persisting one on first use.
/// Publishes the public key next to the DB each time it resolves a key so a verifier can find it.
fn signing_seed() -> Option<[u8; 32]> {
    *SIGNING_SEED.get_or_init(|| {
        // Reuse an existing key.
        if let Ok(Some(hex_str)) = crate::settings::keystore::get_secret(RECEIPT_SIGNING_KEY) {
            if let Ok(bytes) = hex::decode(hex_str.trim()) {
                if let Ok(arr) = <[u8; 32]>::try_from(bytes.as_slice()) {
                    publish_public_key(&SigningKey::from_bytes(&arr).verifying_key());
                    return Some(arr);
                }
            }
        }
        // First run: generate, persist to the keychain, publish the public key.
        let mut seed = [0u8; 32];
        if getrandom::fill(&mut seed).is_err() {
            warn!(target: "4da::engine_runs", "No CSPRNG available — engine_runs receipts will be unsigned");
            return None;
        }
        match crate::settings::keystore::store_secret(RECEIPT_SIGNING_KEY, &hex::encode(seed)) {
            Ok(_) => {
                publish_public_key(&SigningKey::from_bytes(&seed).verifying_key());
                Some(seed)
            }
            Err(e) => {
                warn!(target: "4da::engine_runs", error = %e, "Could not persist receipt signing key — receipts unsigned");
                None
            }
        }
    })
}

/// Write the public key (hex) next to the database for an external verifier to read and pin. The
/// public key is not secret; best-effort.
fn publish_public_key(vk: &VerifyingKey) {
    if let Some(dir) = crate::state::get_db_path()
        .parent()
        .map(std::path::Path::to_path_buf)
    {
        let _ = std::fs::write(
            dir.join("engine_receipt_pubkey.hex"),
            hex::encode(vk.to_bytes()),
        );
    }
}

/// Canonical, versioned, deterministic serialization of a receipt's load-bearing fields. A verifier
/// reconstructs this byte-for-byte from the stored row to verify the signature. Field order and the
/// `v1` prefix are part of the contract — do not reorder without bumping the version.
fn canonical_receipt(
    r: &RunReceipt,
    total: i64,
    watermark: Option<&str>,
    fingerprint: &str,
) -> String {
    format!(
        "v1\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        r.trigger,
        r.completed_at,
        r.new_items,
        r.items_scored,
        total,
        watermark.unwrap_or(""),
        fingerprint,
        r.nonce.as_deref().unwrap_or(""),
    )
}

/// Ed25519-sign the canonical receipt string, returning the hex signature (or `None` if no key).
fn sign_receipt(canonical: &str) -> Option<String> {
    let seed = signing_seed()?;
    let key = SigningKey::from_bytes(&seed);
    Some(hex::encode(key.sign(canonical.as_bytes()).to_bytes()))
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
    /// Optional verifier-assigned task token (from `--nonce`), stamped into the receipt so an
    /// attribution proof can bind this run to a specific task. `None` for unattributed runs.
    pub nonce: Option<String>,
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
            nonce: None,
        }
    }
}

/// Persist a cycle receipt and return the inserted row id (the `engine_run_id`
/// the Upgrade Plan snapshot stamps). Best-effort: a failure here must never
/// break the cycle, so errors are logged and swallowed and `None` is returned.
/// Derives the freshness watermark, source-item total, and content fingerprint
/// from the live DB so the row reflects ground truth rather than the caller's claim.
pub(crate) fn record(mut receipt: RunReceipt) -> Option<i64> {
    if receipt.completed_at.is_empty() {
        receipt.completed_at = now_rfc3339();
    }
    let db = match crate::get_database() {
        Ok(db) => db,
        Err(e) => {
            warn!(target: "4da::engine_runs", error = %e, "Skipping receipt — database unavailable");
            return None;
        }
    };
    let conn = db.conn.lock();
    if let Err(e) = ensure_table(&conn) {
        warn!(target: "4da::engine_runs", error = %e, "Skipping receipt — ensure_table failed");
        return None;
    }

    let (total, watermark) = source_items_state(&conn);
    let fingerprint = fingerprint(total, watermark.as_deref());

    // Sign the receipt's load-bearing fields with the keychain-held private key so a cloned or
    // hand-`INSERT`ed row (a same-user T2 forger) carries no valid signature. Best-effort: an
    // unavailable key records the receipt unsigned rather than failing the cycle.
    let canonical = canonical_receipt(&receipt, total, watermark.as_deref(), &fingerprint);
    let signature = sign_receipt(&canonical);

    let res = conn.execute(
        "INSERT INTO engine_runs (
            trigger, started_at, completed_at, duration_ms,
            sources_succeeded, sources_failed, sources_skipped,
            new_items, cached_touches, items_scored, relevant_count,
            source_items_total, max_item_created_at, content_fingerprint, ok, error, nonce, signature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
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
            receipt.nonce,
            signature,
        ],
    );
    match res {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            debug!(
                target: "4da::engine_runs",
                trigger = receipt.trigger,
                new_items = receipt.new_items,
                scored = receipt.items_scored,
                total,
                ok = receipt.ok,
                run_id = id,
                "Engine-run receipt recorded"
            );
            Some(id)
        }
        Err(e) => {
            warn!(target: "4da::engine_runs", error = %e, "Failed to insert engine-run receipt");
            None
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
// Scoring watermark — the DB-derived differential boundary (2026-08-23 audit, item 9)
// ============================================================================

/// The most recent SUCCESSFUL scoring run, read from `engine_runs`.
///
/// This is what makes differential scoring survive process boundaries: the old
/// gate lived in in-memory `AnalysisState` (`last_completed_at` +
/// `previous_results`), which a fresh `fourda-engine --once` process NEVER has —
/// so every OS-scheduled cycle re-scored the full window (~660 items every 30
/// minutes, measured live 2026-08-23). GUI and headless share the database, so
/// the watermark deliberately accepts ANY trigger.
#[derive(Debug, Clone)]
pub(crate) struct ScoringWatermark {
    /// The run's `started_at` in SQLite-canonical UTC (`%Y-%m-%d %H:%M:%S`) —
    /// the `since` bound for `get_items_since_timestamp_tiered`, which
    /// string-compares against `source_items.last_seen`. Two deliberate choices:
    ///
    /// - **Canonical format, not RFC3339.** The receipt stores RFC3339
    ///   (`2026-08-23T14:04:40+00:00`); SQLite compares TEXT byte-wise, and
    ///   `'T' > ' '`, so an unconverted RFC3339 bound would silently exclude
    ///   every same-day item.
    /// - **`started_at`, not `completed_at`.** Items that arrived while the run
    ///   was scoring were not in its selection; a completed-at bound would skip
    ///   them until the never-scored backfill mopped them up. The cost is
    ///   re-scoring one cycle's worth of overlap, which the persist-boundary
    ///   hysteresis flattens to zero churn.
    pub since_utc: String,
    /// Minutes since the run COMPLETED — the freshness of the watermark.
    /// Negative means the receipt claims a future completion (clock skew);
    /// callers must treat that as "no usable watermark".
    pub completed_age_minutes: i64,
}

/// Read the differential watermark: the newest `engine_runs` row that recorded a
/// successful cycle which actually scored something (`ok = 1 AND items_scored >
/// 0`, any trigger). `None` when no such run exists or its timestamps do not
/// parse — callers fall back to the full window, which is always safe.
pub(crate) fn last_scoring_watermark() -> Option<ScoringWatermark> {
    let db = crate::get_database().ok()?;
    let conn = db.conn.lock();
    ensure_table(&conn).ok()?;
    let (started_at, completed_at): (String, String) = conn
        .query_row(
            "SELECT started_at, completed_at FROM engine_runs
             WHERE ok = 1 AND items_scored > 0
             ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok()?;
    watermark_from_run(&started_at, &completed_at, chrono::Utc::now())
}

/// Parse a receipt timestamp: RFC3339 (what [`now_rfc3339`] writes) with a
/// canonical-format fallback for defensiveness. `None` = unparseable.
fn parse_receipt_time(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|n| n.and_utc())
}

/// Pure core of [`last_scoring_watermark`]: convert one receipt row into a
/// watermark relative to `now`. Split out so the format conversion and age
/// arithmetic are unit-testable without a database.
fn watermark_from_run(
    started_at: &str,
    completed_at: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<ScoringWatermark> {
    let started = parse_receipt_time(started_at)?;
    let completed = parse_receipt_time(completed_at)?;
    Some(ScoringWatermark {
        since_utc: started.format("%Y-%m-%d %H:%M:%S").to_string(),
        completed_age_minutes: now.signed_duration_since(completed).num_minutes(),
    })
}

/// A watermark older than this cannot bound a differential run — after a day
/// of downtime the corpus needs the full window (staleness, decay, drift).
pub(crate) const DIFFERENTIAL_WATERMARK_MAX_AGE_MINUTES: i64 = 24 * 60;

/// The largest pending stale-version drain a differential run may absorb —
/// exactly one drain batch (`get_stale_scored_items`' 500 cap), which the
/// differential path's own `drain_stale_backlog` merge clears in the same run.
/// A bigger backlog means a pipeline-version bump just landed: take the full
/// window so the visible feed converges immediately (and keep taking it until
/// the drain has caught up to within one batch).
pub(crate) const DIFFERENTIAL_MAX_STALE_BACKLOG: i64 = 500;

/// The differential admission rule (audit item 9). Returns the `since` bound
/// when this run may score differentially, `None` for the full window.
///
/// Pure — DB facts come in as arguments — so the decision table is testable:
/// - No watermark / stale (>24h) / future-dated (clock skew) → full window.
/// - Pending stale-version drain beyond one batch (version bump) → full
///   window. The FIRST run after activating a scoring change is therefore
///   full, which is correct — and version-stale items within the bound still
///   drain on differential runs via the existing `drain_stale_backlog` merge.
/// - A non-silent (foreground) run without in-memory previous results → full
///   window: its result set REPLACES the shared display state and the
///   `analysis-complete` feed wholesale, so it must be a complete corpus.
///   Silent runs (scheduled + headless) deliver via the frontend's MERGING
///   `background-results` path (or no UI at all), so they take the
///   differential even from a cold process — that cold headless process is
///   precisely the case the in-memory gate could never serve.
pub(crate) fn differential_since(
    watermark: Option<&ScoringWatermark>,
    stale_backlog: i64,
    has_previous_results: bool,
    merge_delivery: bool,
) -> Option<String> {
    let w = watermark?;
    if w.completed_age_minutes < 0
        || w.completed_age_minutes > DIFFERENTIAL_WATERMARK_MAX_AGE_MINUTES
    {
        return None;
    }
    if stale_backlog > DIFFERENTIAL_MAX_STALE_BACKLOG {
        return None;
    }
    if !merge_delivery && !has_previous_results {
        return None;
    }
    Some(w.since_utc.clone())
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

// ============================================================================
// Tests — the signing contract an external verifier depends on
// ============================================================================
//
// These exercise the canonical-serialization + Ed25519 sign/verify primitives directly with a
// FIXED seed, never the OS keychain (`signing_seed()` is intentionally untouched here). The
// roundtrip mirrors exactly what `sign_receipt()` does on the engine side and what the verifier
// does on the proof side, so it pins the wire contract: same canonical string → verifiable sig;
// any tampered field → verification fails.

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    /// A receipt with deterministic field values for canonical-form assertions.
    fn fixture(nonce: Option<&str>) -> RunReceipt {
        RunReceipt {
            trigger: "headless_once",
            started_at: "2026-06-10T00:00:00+00:00".into(),
            completed_at: "2026-06-10T00:01:00+00:00".into(),
            duration_ms: 60_000,
            sources_succeeded: 20,
            sources_failed: 0,
            sources_skipped: 0,
            new_items: 1202,
            cached_touches: 0,
            items_scored: 916,
            relevant_count: 0,
            ok: true,
            error: None,
            nonce: nonce.map(String::from),
        }
    }

    #[test]
    fn canonical_is_deterministic_versioned_and_binds_nonce() {
        let r = fixture(Some("TASK-7c3a9"));
        let a = canonical_receipt(&r, 1228, Some("2026-06-09 14:02:52"), "abc123");
        let b = canonical_receipt(&r, 1228, Some("2026-06-09 14:02:52"), "abc123");
        assert_eq!(a, b, "canonical form must be deterministic");
        assert!(a.starts_with("v1\n"), "must carry the version prefix");
        assert!(
            a.ends_with("TASK-7c3a9"),
            "nonce must be the bound trailing field"
        );
        // A different nonce must change the signed bytes (anti-replay binding).
        let other = canonical_receipt(
            &fixture(Some("TASK-other")),
            1228,
            Some("2026-06-09 14:02:52"),
            "abc123",
        );
        assert_ne!(
            a, other,
            "a different nonce must produce a different canonical string"
        );
        // A NULL nonce serializes to an empty trailing field (still well-formed).
        let null_nonce =
            canonical_receipt(&fixture(None), 1228, Some("2026-06-09 14:02:52"), "abc123");
        assert!(
            null_nonce.ends_with('\n'),
            "absent nonce → empty trailing field"
        );
    }

    #[test]
    fn sign_then_verify_with_published_pubkey_roundtrips() {
        // Mirror sign_receipt() with a fixed seed (no keychain): this is the exact byte path the
        // engine signs and the verifier checks.
        let seed = [7u8; 32];
        let key = SigningKey::from_bytes(&seed);
        let canonical = canonical_receipt(
            &fixture(Some("TASK-7c3a9")),
            1228,
            Some("2026-06-09 14:02:52"),
            "abc123",
        );

        let sig_hex = hex::encode(key.sign(canonical.as_bytes()).to_bytes());

        // Verifier side: reconstruct the same canonical string, decode the published pubkey + sig.
        let vk = key.verifying_key();
        let sig_bytes: [u8; 64] = hex::decode(&sig_hex).unwrap().try_into().unwrap();
        let sig = Signature::from_bytes(&sig_bytes);
        assert!(
            vk.verify(canonical.as_bytes(), &sig).is_ok(),
            "honest receipt must verify"
        );
    }

    #[test]
    fn tampered_field_fails_verification() {
        let seed = [7u8; 32];
        let key = SigningKey::from_bytes(&seed);
        let honest = canonical_receipt(
            &fixture(Some("TASK-7c3a9")),
            1228,
            Some("2026-06-09 14:02:52"),
            "abc123",
        );
        let sig = key.sign(honest.as_bytes());
        let vk = key.verifying_key();

        // Forge a richer-looking row: bump items_scored. A T2 adversary editing the DB row this way
        // changes the canonical string, so the original signature no longer verifies.
        let mut forged = fixture(Some("TASK-7c3a9"));
        forged.items_scored = 999_999;
        let forged_canonical =
            canonical_receipt(&forged, 1228, Some("2026-06-09 14:02:52"), "abc123");
        assert!(
            vk.verify(forged_canonical.as_bytes(), &sig).is_err(),
            "a tampered field must invalidate the signature"
        );

        // Re-using the signature against a different nonce (replay) also fails.
        let replay = canonical_receipt(
            &fixture(Some("TASK-stolen")),
            1228,
            Some("2026-06-09 14:02:52"),
            "abc123",
        );
        assert!(
            vk.verify(replay.as_bytes(), &sig).is_err(),
            "replaying a signature under a fresh task nonce must fail"
        );
    }

    // ------------------------------------------------------------------
    // Scoring watermark (2026-08-23 audit, item 9)
    // ------------------------------------------------------------------

    fn utc(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_utc()
    }

    /// The receipt stores RFC3339; the watermark MUST come out in SQLite
    /// canonical form. `'T' > ' '` in byte-wise TEXT comparison, so an
    /// unconverted bound would exclude every same-day item from the
    /// differential selection — silently.
    #[test]
    fn watermark_converts_rfc3339_to_sqlite_canonical_form() {
        let w = watermark_from_run(
            "2026-08-23T14:04:40.123456+00:00",
            "2026-08-23T14:09:40+00:00",
            utc("2026-08-23 14:39:40"),
        )
        .expect("valid receipt row must yield a watermark");
        assert_eq!(w.since_utc, "2026-08-23 14:04:40");
        assert_eq!(w.completed_age_minutes, 30);
        // The load-bearing property: the canonical form sorts BELOW a
        // later same-day canonical `last_seen`, where raw RFC3339 would not.
        assert!(w.since_utc.as_str() < "2026-08-23 15:00:00");
        assert!("2026-08-23T14:04:40+00:00" > "2026-08-23 15:00:00");
    }

    /// Non-UTC offsets normalize to UTC before formatting — the DB's
    /// `last_seen` is UTC, so a local-offset watermark would shift the
    /// differential window by hours.
    #[test]
    fn watermark_normalizes_offsets_to_utc() {
        let w = watermark_from_run(
            "2026-08-24T00:04:40+10:00",
            "2026-08-24T00:09:40+10:00",
            utc("2026-08-23 14:39:40"),
        )
        .unwrap();
        assert_eq!(w.since_utc, "2026-08-23 14:04:40");
        assert_eq!(w.completed_age_minutes, 30);
    }

    /// Unparseable timestamps yield no watermark (the caller falls back to
    /// the full window), and a future completion surfaces as a NEGATIVE age
    /// the caller must reject rather than treat as ultra-fresh.
    #[test]
    fn watermark_rejects_garbage_and_flags_clock_skew() {
        assert!(watermark_from_run(
            "not-a-date",
            "2026-08-23T14:09:40+00:00",
            utc("2026-08-23 15:00:00")
        )
        .is_none());
        assert!(watermark_from_run(
            "2026-08-23T14:04:40+00:00",
            "garbage",
            utc("2026-08-23 15:00:00")
        )
        .is_none());
        let skewed = watermark_from_run(
            "2026-08-23T14:04:40+00:00",
            "2026-08-23T16:00:00+00:00",
            utc("2026-08-23 15:00:00"),
        )
        .unwrap();
        assert!(
            skewed.completed_age_minutes < 0,
            "future completion must read as negative age"
        );
    }

    /// The canonical-format fallback parses legacy rows too.
    #[test]
    fn watermark_accepts_canonical_format_rows() {
        let w = watermark_from_run(
            "2026-08-23 14:04:40",
            "2026-08-23 14:09:40",
            utc("2026-08-23 14:19:40"),
        )
        .unwrap();
        assert_eq!(w.since_utc, "2026-08-23 14:04:40");
        assert_eq!(w.completed_age_minutes, 10);
    }

    // ------------------------------------------------------------------
    // Differential admission rule (item 9's decision table)
    // ------------------------------------------------------------------

    fn wm(age_minutes: i64) -> ScoringWatermark {
        ScoringWatermark {
            since_utc: "2026-08-23 14:04:40".into(),
            completed_age_minutes: age_minutes,
        }
    }

    /// A recent watermark and no pending drain admit the differential path —
    /// for a cold silent process (headless: no in-memory previous results),
    /// which is exactly what the old in-memory gate could never grant.
    #[test]
    fn recent_watermark_and_clean_backlog_admit_differential() {
        let since = differential_since(Some(&wm(30)), 0, false, true);
        assert_eq!(since.as_deref(), Some("2026-08-23 14:04:40"));
        // With in-memory previous results a foreground run qualifies too.
        assert!(differential_since(Some(&wm(30)), 0, true, false).is_some());
    }

    /// A stale (>24h), missing, or future-dated watermark falls back to the
    /// full window.
    #[test]
    fn stale_missing_or_skewed_watermark_takes_full_window() {
        assert!(differential_since(None, 0, true, true).is_none());
        assert!(differential_since(Some(&wm(24 * 60 + 1)), 0, true, true).is_none());
        assert!(differential_since(Some(&wm(-5)), 0, true, true).is_none());
        assert!(differential_since(Some(&wm(24 * 60)), 0, true, true).is_some());
    }

    /// A pending stale-version drain within one batch rides along on the
    /// differential run (its drain merge clears it); beyond one batch — a
    /// pipeline bump just landed — the run takes the full window.
    #[test]
    fn version_bump_backlog_forces_full_window_small_backlog_rides_along() {
        assert!(
            differential_since(Some(&wm(30)), DIFFERENTIAL_MAX_STALE_BACKLOG, false, true)
                .is_some()
        );
        assert!(differential_since(
            Some(&wm(30)),
            DIFFERENTIAL_MAX_STALE_BACKLOG + 1,
            false,
            true
        )
        .is_none());
    }

    /// A foreground run without in-memory previous results REPLACES the
    /// visible feed, so it may never return a partial differential set.
    #[test]
    fn cold_foreground_run_takes_full_window() {
        assert!(differential_since(Some(&wm(30)), 0, false, false).is_none());
    }
}

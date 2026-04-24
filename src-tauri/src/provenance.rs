// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Intelligence Mesh — Provenance Graph (Layer 5).
//!
//! Every AI-influenced artifact in 4DA carries provenance: which model
//! produced it, under which prompt version, against which calibration,
//! at what temperature. Provenance is load-bearing — without it, scores
//! from different models are silently incomparable, compound-learning
//! imports model drift, and auditing a bad recommendation is impossible.
//!
//! See `docs/strategy/INTELLIGENCE-MESH.md` §5 for the full design.
//!
//! This module provides:
//!   • `ArtifactKind` — the enumeration of provenance-carrying artifacts
//!   • `Provenance` — the full record persisted to the `provenance` table
//!   • `ModelIdentity` — identity-hashing for stable cross-reference keys
//!   • `record()` / `latest_for_artifact()` — SQL helpers
//!   • `PRE_MESH_CALIBRATION_ID` — sentinel for artifacts produced before
//!     the mesh pivot; compound-learning treats these as a distinct cohort

use crate::error::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

/// Sentinel value for `calibration_id` when an artifact was produced before
/// per-model calibration existed (pre-mesh-pivot data, or a run where the
/// model hasn't completed the calibration battery yet).
///
/// Compound-learning loops treat artifacts with this sentinel as a separate
/// cohort from calibrated artifacts — they don't meaningfully normalize and
/// must not be compared against post-calibration scores.
pub const PRE_MESH_CALIBRATION_ID: &str = "pre-mesh-unknown";

/// Categories of AI-influenced artifacts that carry provenance.
///
/// Adding a new variant is a semver-adjacent change: the underlying text
/// value is written to SQL and must remain stable across releases. When
/// adding a variant, also update `as_sql_str` and `from_sql_str`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub enum ArtifactKind {
    /// Final relevance score assigned to a source_item.
    Score,
    /// LLM reranking adjustment applied to a score.
    Rerank,
    /// LLM-generated per-item summary (content_commands).
    Summary,
    /// Morning briefing synthesis prose (monitoring).
    Briefing,
    /// Intelligence Console search synthesis output.
    SearchSynthesis,
    /// Channel overview synthesis.
    ChannelOverview,
    /// Translated UI / content strings.
    Translation,
    /// Embedding vector generation.
    Embed,
    /// User-initiated chat reply.
    Chat,
}

impl ArtifactKind {
    /// Stable SQL string representation. Changing these requires a migration.
    pub fn as_sql_str(&self) -> &'static str {
        match self {
            ArtifactKind::Score => "score",
            ArtifactKind::Rerank => "rerank",
            ArtifactKind::Summary => "summary",
            ArtifactKind::Briefing => "briefing",
            ArtifactKind::SearchSynthesis => "search_synthesis",
            ArtifactKind::ChannelOverview => "channel_overview",
            ArtifactKind::Translation => "translation",
            ArtifactKind::Embed => "embed",
            ArtifactKind::Chat => "chat",
        }
    }

    pub fn from_sql_str(s: &str) -> Option<Self> {
        match s {
            "score" => Some(ArtifactKind::Score),
            "rerank" => Some(ArtifactKind::Rerank),
            "summary" => Some(ArtifactKind::Summary),
            "briefing" => Some(ArtifactKind::Briefing),
            "search_synthesis" => Some(ArtifactKind::SearchSynthesis),
            "channel_overview" => Some(ArtifactKind::ChannelOverview),
            "translation" => Some(ArtifactKind::Translation),
            "embed" => Some(ArtifactKind::Embed),
            "chat" => Some(ArtifactKind::Chat),
            _ => None,
        }
    }
}

/// Stable identity for a model instance, used as a cross-reference key
/// between provenance rows, calibration curves, and shadow-arena peers.
///
/// The hash is `sha256(provider + ":" + model + ":" + base_url)`. Two
/// models with the same provider/name served from different base_urls
/// (e.g. Ollama on two different hosts) get distinct identities — which
/// is correct, because they are different runtime entities and may have
/// different calibrations.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct ModelIdentity {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
}

impl ModelIdentity {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            base_url: None,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Hex-encoded SHA-256 identity hash. Stable across releases.
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.provider.as_bytes());
        hasher.update(b":");
        hasher.update(self.model.as_bytes());
        hasher.update(b":");
        hasher.update(self.base_url.as_deref().unwrap_or("").as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// One persisted provenance record. Maps 1:1 to a row in the `provenance`
/// SQL table created by Phase 56 migration.
///
/// The `id` is `None` before insert and `Some(rowid)` after. Callers
/// typically don't need the id, but shadow-arena code uses it to link
/// shadow-peer rows via `shadow_peer_id`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct Provenance {
    pub id: Option<i64>,
    pub artifact_kind: ArtifactKind,
    /// String representation of the artifact's primary key. For
    /// source_items this is the i64 id stringified; for summaries/
    /// briefings this is the DB rowid; for synthetic kinds (embed,
    /// chat) this may be any stable identifier the caller provides.
    pub artifact_id: String,
    pub model_identity_hash: String,
    pub provider: String,
    pub model: String,
    /// Versioned prompt identifier, e.g. "judge-v1-2026-04-15".
    pub prompt_version: Option<String>,
    /// Sentinel `PRE_MESH_CALIBRATION_ID` for uncalibrated artifacts.
    pub calibration_id: Option<String>,
    /// Task performed. Stable short string ("judge", "rerank", etc.).
    pub task: String,
    pub temperature: Option<f32>,
    /// Optional audit trail. SHA-256 of the raw model response. Present
    /// only when audit_retention_days > 0 in settings (not implemented
    /// yet — reserved for future audit feature).
    pub raw_response_hash: Option<String>,
    /// If this row represents a shadow-arena run, the id of the peer
    /// (baseline) provenance row it was compared against. The baseline
    /// row itself has shadow_peer_id = None.
    pub shadow_peer_id: Option<i64>,
    /// RFC3339 / SQLite `datetime('now')` timestamp. Populated by the
    /// DB default when None is passed to `record`.
    pub created_at: Option<String>,
}

impl Provenance {
    /// Build a new (un-persisted) Provenance record.
    pub fn new(
        kind: ArtifactKind,
        artifact_id: impl Into<String>,
        identity: &ModelIdentity,
        task: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            artifact_kind: kind,
            artifact_id: artifact_id.into(),
            model_identity_hash: identity.hash(),
            provider: identity.provider.clone(),
            model: identity.model.clone(),
            prompt_version: None,
            calibration_id: Some(PRE_MESH_CALIBRATION_ID.to_string()),
            task: task.into(),
            temperature: None,
            raw_response_hash: None,
            shadow_peer_id: None,
            created_at: None,
        }
    }

    pub fn with_prompt_version(mut self, v: impl Into<String>) -> Self {
        self.prompt_version = Some(v.into());
        self
    }

    pub fn with_calibration_id(mut self, id: impl Into<String>) -> Self {
        self.calibration_id = Some(id.into());
        self
    }

    pub fn with_temperature(mut self, t: f32) -> Self {
        self.temperature = Some(t);
        self
    }

    pub fn with_shadow_peer(mut self, peer_id: i64) -> Self {
        self.shadow_peer_id = Some(peer_id);
        self
    }
}

/// Insert a `Provenance` row. Returns the rowid.
///
/// This is the single write path for provenance. All stamping code paths
/// should route through it so indexing and timestamp discipline remain
/// consistent. Never write to the `provenance` table directly from other
/// modules — add a helper here instead.
pub fn record(conn: &Connection, prov: &Provenance) -> Result<i64> {
    conn.execute(
        "INSERT INTO provenance (
            artifact_kind, artifact_id, model_identity_hash,
            provider, model, prompt_version, calibration_id,
            task, temperature, raw_response_hash, shadow_peer_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            prov.artifact_kind.as_sql_str(),
            prov.artifact_id,
            prov.model_identity_hash,
            prov.provider,
            prov.model,
            prov.prompt_version,
            prov.calibration_id,
            prov.task,
            prov.temperature,
            prov.raw_response_hash,
            prov.shadow_peer_id,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert many `Provenance` rows atomically. Use for batch operations
/// (e.g. rerank stamping N items at once). Returns inserted rowids in
/// the same order as input.
pub fn record_batch(conn: &Connection, provs: &[Provenance]) -> Result<Vec<i64>> {
    let tx = conn.unchecked_transaction()?;
    let mut ids = Vec::with_capacity(provs.len());
    for prov in provs {
        tx.execute(
            "INSERT INTO provenance (
                artifact_kind, artifact_id, model_identity_hash,
                provider, model, prompt_version, calibration_id,
                task, temperature, raw_response_hash, shadow_peer_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                prov.artifact_kind.as_sql_str(),
                prov.artifact_id,
                prov.model_identity_hash,
                prov.provider,
                prov.model,
                prov.prompt_version,
                prov.calibration_id,
                prov.task,
                prov.temperature,
                prov.raw_response_hash,
                prov.shadow_peer_id,
            ],
        )?;
        ids.push(tx.last_insert_rowid());
    }
    tx.commit()?;
    Ok(ids)
}

/// Look up the most recent provenance row for a given artifact. Returns
/// `None` if the artifact has no provenance (pre-mesh data, or never
/// AI-influenced).
pub fn latest_for_artifact(
    conn: &Connection,
    kind: ArtifactKind,
    artifact_id: &str,
) -> Result<Option<Provenance>> {
    conn.query_row(
        "SELECT id, artifact_kind, artifact_id, model_identity_hash,
                provider, model, prompt_version, calibration_id,
                task, temperature, raw_response_hash, shadow_peer_id,
                created_at
         FROM provenance
         WHERE artifact_kind = ?1 AND artifact_id = ?2
         ORDER BY id DESC
         LIMIT 1",
        params![kind.as_sql_str(), artifact_id],
        |row| {
            Ok(Provenance {
                id: row.get(0)?,
                artifact_kind: ArtifactKind::from_sql_str(&row.get::<_, String>(1)?)
                    .unwrap_or(ArtifactKind::Score),
                artifact_id: row.get(2)?,
                model_identity_hash: row.get(3)?,
                provider: row.get(4)?,
                model: row.get(5)?,
                prompt_version: row.get(6)?,
                calibration_id: row.get(7)?,
                task: row.get(8)?,
                temperature: row.get(9)?,
                raw_response_hash: row.get(10)?,
                shadow_peer_id: row.get(11)?,
                created_at: row.get(12)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// Count provenance rows for a given model identity. Used by shadow-arena
/// promotion criteria (e.g. "at least 50 runs before graduating").
pub fn count_for_model(conn: &Connection, model_identity_hash: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM provenance WHERE model_identity_hash = ?1",
        params![model_identity_hash],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Create an in-memory connection with the provenance table schema.
    /// Matches the Phase 56 migration exactly. If the migration ever drifts,
    /// this test helper must be updated in lockstep.
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE provenance (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                artifact_kind TEXT NOT NULL,
                artifact_id TEXT NOT NULL,
                model_identity_hash TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                prompt_version TEXT,
                calibration_id TEXT,
                task TEXT NOT NULL,
                temperature REAL,
                raw_response_hash TEXT,
                shadow_peer_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE INDEX idx_provenance_artifact ON provenance(artifact_kind, artifact_id);
             CREATE INDEX idx_provenance_model ON provenance(model_identity_hash);
             CREATE INDEX idx_provenance_created_at ON provenance(created_at);
             CREATE INDEX idx_provenance_task ON provenance(task);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn artifact_kind_sql_roundtrip() {
        let all = [
            ArtifactKind::Score,
            ArtifactKind::Rerank,
            ArtifactKind::Summary,
            ArtifactKind::Briefing,
            ArtifactKind::SearchSynthesis,
            ArtifactKind::ChannelOverview,
            ArtifactKind::Translation,
            ArtifactKind::Embed,
            ArtifactKind::Chat,
        ];
        for kind in all {
            let s = kind.as_sql_str();
            let back = ArtifactKind::from_sql_str(s).expect("roundtrip");
            assert_eq!(kind, back, "roundtrip failed for {:?}", kind);
        }
    }

    #[test]
    fn artifact_kind_unknown_string_returns_none() {
        assert!(ArtifactKind::from_sql_str("not-a-real-kind").is_none());
    }

    #[test]
    fn model_identity_hash_is_deterministic() {
        let a = ModelIdentity::new("ollama", "llama3.2");
        let b = ModelIdentity::new("ollama", "llama3.2");
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn model_identity_hash_differs_across_models() {
        let a = ModelIdentity::new("ollama", "llama3.2");
        let b = ModelIdentity::new("ollama", "qwen2.5");
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn model_identity_hash_differs_across_providers() {
        let a = ModelIdentity::new("ollama", "llama3.2");
        let b = ModelIdentity::new("anthropic", "llama3.2");
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn model_identity_hash_differs_across_base_urls() {
        let a = ModelIdentity::new("ollama", "llama3.2").with_base_url("http://localhost:11434");
        let b = ModelIdentity::new("ollama", "llama3.2").with_base_url("http://other:11434");
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn model_identity_hash_is_hex_sha256_length() {
        // 256 bits / 4 bits per hex char = 64
        let id = ModelIdentity::new("ollama", "llama3.2");
        assert_eq!(id.hash().len(), 64);
        assert!(id.hash().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn provenance_defaults_to_pre_mesh_calibration() {
        let identity = ModelIdentity::new("ollama", "llama3.2");
        let prov = Provenance::new(ArtifactKind::Score, "42", &identity, "judge");
        assert_eq!(
            prov.calibration_id.as_deref(),
            Some(PRE_MESH_CALIBRATION_ID)
        );
    }

    #[test]
    fn record_and_read_back() {
        let conn = test_conn();
        let identity = ModelIdentity::new("ollama", "llama3.2");
        let prov = Provenance::new(ArtifactKind::Rerank, "source-7", &identity, "rerank")
            .with_prompt_version("judge-v1-2026-04-15")
            .with_temperature(0.2);
        let id = record(&conn, &prov).expect("record");
        assert!(id > 0);

        let back = latest_for_artifact(&conn, ArtifactKind::Rerank, "source-7")
            .expect("query")
            .expect("row");
        assert_eq!(back.id, Some(id));
        assert_eq!(back.artifact_kind, ArtifactKind::Rerank);
        assert_eq!(back.artifact_id, "source-7");
        assert_eq!(back.provider, "ollama");
        assert_eq!(back.model, "llama3.2");
        assert_eq!(back.prompt_version.as_deref(), Some("judge-v1-2026-04-15"));
        assert_eq!(back.task, "rerank");
        assert!(back.temperature.is_some());
        assert!(back.created_at.is_some());
    }

    #[test]
    fn latest_for_artifact_returns_most_recent() {
        let conn = test_conn();
        let identity = ModelIdentity::new("ollama", "llama3.2");

        // Two provenance rows for the same artifact — an older rerank and a
        // newer one. `latest_for_artifact` must return the newer (higher id).
        let p1 = Provenance::new(ArtifactKind::Rerank, "source-1", &identity, "rerank")
            .with_prompt_version("judge-v1");
        let p2 = Provenance::new(ArtifactKind::Rerank, "source-1", &identity, "rerank")
            .with_prompt_version("judge-v2");

        let id1 = record(&conn, &p1).unwrap();
        let id2 = record(&conn, &p2).unwrap();
        assert!(id2 > id1);

        let latest = latest_for_artifact(&conn, ArtifactKind::Rerank, "source-1")
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, Some(id2));
        assert_eq!(latest.prompt_version.as_deref(), Some("judge-v2"));
    }

    #[test]
    fn latest_for_artifact_returns_none_when_missing() {
        let conn = test_conn();
        let result = latest_for_artifact(&conn, ArtifactKind::Score, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn record_batch_inserts_all_atomically() {
        let conn = test_conn();
        let identity = ModelIdentity::new("ollama", "llama3.2");
        let provs: Vec<Provenance> = (0..5)
            .map(|i| {
                Provenance::new(
                    ArtifactKind::Score,
                    format!("item-{}", i),
                    &identity,
                    "judge",
                )
            })
            .collect();
        let ids = record_batch(&conn, &provs).expect("batch");
        assert_eq!(ids.len(), 5);
        // All ids are unique and strictly increasing.
        for window in ids.windows(2) {
            assert!(window[1] > window[0]);
        }
        let count = count_for_model(&conn, &identity.hash()).unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn count_for_model_scopes_to_identity() {
        let conn = test_conn();
        let llama = ModelIdentity::new("ollama", "llama3.2");
        let qwen = ModelIdentity::new("ollama", "qwen2.5");
        record(
            &conn,
            &Provenance::new(ArtifactKind::Score, "a", &llama, "judge"),
        )
        .unwrap();
        record(
            &conn,
            &Provenance::new(ArtifactKind::Score, "b", &llama, "judge"),
        )
        .unwrap();
        record(
            &conn,
            &Provenance::new(ArtifactKind::Score, "c", &qwen, "judge"),
        )
        .unwrap();

        assert_eq!(count_for_model(&conn, &llama.hash()).unwrap(), 2);
        assert_eq!(count_for_model(&conn, &qwen.hash()).unwrap(), 1);
    }

    #[test]
    fn shadow_peer_linkage_preserved() {
        let conn = test_conn();
        let baseline = ModelIdentity::new("ollama", "llama3.2");
        let candidate = ModelIdentity::new("ollama", "qwen2.5");

        let base_prov = Provenance::new(ArtifactKind::Score, "item-1", &baseline, "judge");
        let base_id = record(&conn, &base_prov).unwrap();

        let cand_prov = Provenance::new(ArtifactKind::Score, "item-1", &candidate, "judge")
            .with_shadow_peer(base_id);
        let cand_id = record(&conn, &cand_prov).unwrap();

        let back = latest_for_artifact(&conn, ArtifactKind::Score, "item-1")
            .unwrap()
            .unwrap();
        // latest is the candidate (most recent id).
        assert_eq!(back.id, Some(cand_id));
        assert_eq!(back.shadow_peer_id, Some(base_id));
    }
}

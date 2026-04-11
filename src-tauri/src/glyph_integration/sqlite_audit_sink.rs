//! Real SQLite-backed [`AuditSink`] — Phase 2 audit-only storage.
//!
//! This is the 4DA production implementation. It shares the same schema
//! as the glyph-integration-harness reference implementation in the
//! glyph repo, adapted for 4DA's `rusqlite = "0.32"` (vs harness's 0.31)
//! and 4DA's strict clippy lints (`unwrap_used = deny`, `expect_used = warn`).
//!
//! ## Connection strategy
//!
//! Phase 2 opens its own SQLite connection to the 4DA database file. It
//! does NOT share the existing `Database` handle — that's intentional,
//! because:
//!
//! 1. The audit sink's writes are fire-and-forget (no need for the main
//!    connection's transaction semantics)
//! 2. Separation keeps the glyph_integration module self-contained and
//!    testable without pulling in all of 4DA's `db::Database` machinery
//! 3. Read queries (`find_by_id`, `recent_for_agent`) are dashboard
//!    operations that benefit from a separate connection anyway
//!
//! SQLite supports multiple connections to the same file via WAL mode,
//! which 4DA already enables in migrations phase 1.

use std::path::Path;
use std::sync::Mutex;

use chrono::DateTime;
use glyph_core::EnvelopeId;
use glyph_safety::audit::{AuditEntry, AuditLevel, AuditSink};
use glyph_safety::report::ValidationVerdict;
use rusqlite::{params, Connection, OptionalExtension};

/// Real SQLite audit sink for the 4DA `glyph_audit` table.
pub struct SqliteAuditSink {
    conn: Mutex<Connection>,
}

impl SqliteAuditSink {
    /// Open an audit sink backed by a path to the 4DA SQLite database.
    ///
    /// The caller supplies the path to `data/4da.db` (or equivalent).
    /// The `glyph_audit` table must already have been created via
    /// migration 54 — this constructor does NOT run migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("sqlite open: {e}"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory sink. Used for tests only.
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("sqlite memory: {e}"))?;
        // For in-memory sinks we DO initialise the schema since there's
        // no migration pipeline.
        crate::glyph_integration::migration::create_glyph_audit_table(&conn)
            .map_err(|e| format!("create schema: {e}"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn verdict_to_str(v: ValidationVerdict) -> &'static str {
        match v {
            ValidationVerdict::Propagate => "propagate",
            ValidationVerdict::AwaitingHumanAck => "awaiting_human_ack",
            ValidationVerdict::AwaitingConsequenceScan => "awaiting_consequence_scan",
            ValidationVerdict::Rejected => "rejected",
        }
    }

    fn verdict_from_str(s: &str) -> ValidationVerdict {
        match s {
            "propagate" => ValidationVerdict::Propagate,
            "awaiting_human_ack" => ValidationVerdict::AwaitingHumanAck,
            "awaiting_consequence_scan" => ValidationVerdict::AwaitingConsequenceScan,
            _ => ValidationVerdict::Rejected,
        }
    }

    fn level_to_str(l: AuditLevel) -> &'static str {
        match l {
            AuditLevel::Standard => "standard",
            AuditLevel::Elevated => "elevated",
        }
    }

    fn level_from_str(s: &str) -> AuditLevel {
        if s == "elevated" {
            AuditLevel::Elevated
        } else {
            AuditLevel::Standard
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row_to_entry(
        envelope_id: String,
        agent: String,
        logged_at_str: String,
        summary: String,
        compiled_nl: String,
        header_glyphs: String,
        verdict_str: String,
        level_str: String,
    ) -> AuditEntry {
        let logged_at = DateTime::parse_from_rfc3339(&logged_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        AuditEntry {
            envelope_id: EnvelopeId::from_raw(envelope_id),
            agent,
            logged_at,
            summary,
            compiled_nl,
            header_glyphs,
            verdict: Self::verdict_from_str(&verdict_str),
            level: Self::level_from_str(&level_str),
        }
    }
}

impl AuditSink for SqliteAuditSink {
    fn write(&mut self, entry: &AuditEntry) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock: {e}"))?;
        conn.execute(
            r#"
            INSERT INTO glyph_audit
                (envelope_id, agent, logged_at, summary, compiled_nl, header_glyphs, verdict, level, payload_bytes)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                entry.envelope_id.0,
                entry.agent,
                entry.logged_at.to_rfc3339(),
                entry.summary,
                entry.compiled_nl,
                entry.header_glyphs,
                Self::verdict_to_str(entry.verdict),
                Self::level_to_str(entry.level),
                entry.compiled_nl.len() as i64,
            ],
        )
        .map_err(|e| format!("insert: {e}"))?;
        Ok(())
    }

    fn find_by_id(&self, id: &EnvelopeId) -> Vec<AuditEntry> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT envelope_id, agent, logged_at, summary, compiled_nl, header_glyphs, verdict, level
             FROM glyph_audit WHERE envelope_id = ?1 ORDER BY id",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![id.0], |row| {
            Ok(Self::row_to_entry(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn recent_for_agent(&self, agent: &str, limit: usize) -> Vec<AuditEntry> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT envelope_id, agent, logged_at, summary, compiled_nl, header_glyphs, verdict, level
             FROM glyph_audit WHERE agent = ?1 ORDER BY id DESC LIMIT ?2",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![agent, limit as i64], |row| {
            Ok(Self::row_to_entry(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn len(&self) -> usize {
        let Ok(conn) = self.conn.lock() else {
            return 0;
        };
        conn.query_row("SELECT COUNT(*) FROM glyph_audit", params![], |row| {
            row.get::<_, i64>(0)
        })
        .optional()
        .ok()
        .flatten()
        .map(|n| n as usize)
        .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn fake_entry(agent: &str, id: &str) -> AuditEntry {
        AuditEntry {
            envelope_id: EnvelopeId::from_raw(id),
            agent: agent.to_string(),
            logged_at: Utc::now(),
            summary: "[web·high·implies·infra·reversible·ok] a: test".to_string(),
            compiled_nl: "[web] [high] [implies] in domain [infra] with reversibility [reversible] and risk [ok].\n\nPayload: test".to_string(),
            header_glyphs: "🌐·●·➜·⚙·↶·🟢".to_string(),
            verdict: ValidationVerdict::Propagate,
            level: AuditLevel::Standard,
        }
    }

    #[test]
    fn in_memory_write_and_read() {
        let mut sink = match SqliteAuditSink::open_in_memory() {
            Ok(s) => s,
            Err(e) => panic!("open_in_memory: {e}"),
        };
        assert_eq!(sink.len(), 0);
        let entry = fake_entry("test-agent", "id-1");
        if let Err(e) = sink.write(&entry) {
            panic!("write: {e}");
        }
        assert_eq!(sink.len(), 1);

        let found = sink.find_by_id(&EnvelopeId::from_raw("id-1"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].agent, "test-agent");
    }

    #[test]
    fn recent_for_agent_orders_newest_first() {
        let mut sink = match SqliteAuditSink::open_in_memory() {
            Ok(s) => s,
            Err(e) => panic!("open_in_memory: {e}"),
        };
        for i in 0..5 {
            if let Err(e) = sink.write(&fake_entry("a", &format!("id-{i}"))) {
                panic!("write: {e}");
            }
        }
        if let Err(e) = sink.write(&fake_entry("b", "id-b1")) {
            panic!("write: {e}");
        }
        let recent = sink.recent_for_agent("a", 3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].envelope_id.0, "id-4");
        assert_eq!(recent[1].envelope_id.0, "id-3");
        assert_eq!(recent[2].envelope_id.0, "id-2");
    }
}

//! Shadow-envelope wrapper for MCP tool calls.
//!
//! In Phase 2 audit-only mode, every MCP tool call that flows through
//! 4DA's broker gets a **shadow envelope** generated alongside it. The
//! envelope is built with `glyph-lift` in `Defaulted` policy (so every
//! call produces one — never `None`), validated by `glyph-safety`, and
//! written to the `glyph_audit` table via `SqliteAuditSink`.
//!
//! The tool call itself is **not modified**. The wrapper is a pure
//! observer. This lets us collect Phase 2 measurements (categorical
//! coverage, audit growth, verdict distribution) without any behaviour
//! change for agents or users.
//!
//! ## Phase 3 evolution
//!
//! In Phase 3 one agent (`gotcha-detector`) starts emitting real envelopes
//! alongside its NL output. The wrapper here stays the same — it keeps
//! shadowing every OTHER tool call — and a separate path in the broker
//! handles the explicit envelope emission.
//!
//! ## Phase 4 evolution
//!
//! In Phase 4 the wrapper becomes a real broker: the envelope's glyph
//! header drives routing, and the tool call only proceeds after
//! `glyph-safety::validate` returns `Propagate`. At that point the
//! wrapper stops being a pure observer — but that's Phase 4, not Phase 2.

use chrono::Utc;
use glyph_core::{Dictionary, Envelope, EnvelopeId, EnvelopeMeta, Payload};
use glyph_lift::{lift, LiftPolicy};
use glyph_safety::{
    capability::CapabilityRegistry, validate, AntiStegMonitor, AuditSink, SafetyContext,
};

use super::wisdom_bridge::{NoopAck, NoopAwe};

/// Result of building and validating a shadow envelope.
#[derive(Debug)]
pub struct ShadowEnvelopeResult {
    /// The envelope that was built (if one could be built).
    pub envelope: Option<Envelope>,
    /// Whether the envelope was successfully written to the audit sink.
    pub audit_written: bool,
    /// Brief human-readable status for logging.
    pub status: String,
}

/// Builder for shadow envelopes. Holds references to the dictionary,
/// capability registry, and anti-steg monitor so they persist across
/// many calls without per-call re-initialisation.
pub struct ShadowEnvelopeBuilder<'a> {
    dict: &'a Dictionary,
    capabilities: &'a CapabilityRegistry,
}

impl<'a> ShadowEnvelopeBuilder<'a> {
    /// Construct a builder over an active dictionary and capability registry.
    pub fn new(dict: &'a Dictionary, capabilities: &'a CapabilityRegistry) -> Self {
        Self {
            dict,
            capabilities,
        }
    }

    /// Wrap an MCP tool call with a shadow envelope and write it to the
    /// audit sink. Returns a [`ShadowEnvelopeResult`] for logging but
    /// does NOT change the caller's behaviour — the tool call itself
    /// completes normally.
    ///
    /// Arguments:
    /// - `agent` — the calling agent's id (free-form string)
    /// - `tool_name` — the MCP tool name (becomes part of the NL payload)
    /// - `tool_summary` — a one-line human-readable summary of what the tool did
    /// - `audit_sink` — the `SqliteAuditSink` to write the envelope into
    /// - `anti_steg` — a persistent `AntiStegMonitor` shared across all
    ///   builder invocations (stateful, accumulates per-agent history)
    pub fn wrap_mcp_call(
        &self,
        agent: &str,
        tool_name: &str,
        tool_summary: &str,
        audit_sink: &mut dyn AuditSink,
        anti_steg: &mut AntiStegMonitor,
    ) -> ShadowEnvelopeResult {
        // Lift the NL summary into an envelope. Defaulted policy always
        // produces one (never None) — it falls back to safe defaults when
        // a position isn't explicitly signalled by the text.
        let text = format!("{tool_name}: {tool_summary}");
        let lifted = match lift(&text, self.dict, LiftPolicy::Defaulted, agent, None) {
            Ok(Some(env)) => env,
            Ok(None) => {
                return ShadowEnvelopeResult {
                    envelope: None,
                    audit_written: false,
                    status: format!("lift produced no envelope for tool {tool_name}"),
                };
            }
            Err(e) => {
                return ShadowEnvelopeResult {
                    envelope: None,
                    audit_written: false,
                    status: format!("lift error: {e}"),
                };
            }
        };

        // Overwrite metadata with the real agent id + timestamp.
        // (Defaulted lift fills in its own, but we want the caller's values.)
        let envelope = Envelope {
            meta: EnvelopeMeta {
                id: EnvelopeId::new(),
                timestamp: Utc::now(),
                agent: agent.to_string(),
                dict_version: self.dict.version().to_string(),
                parent: None,
            },
            payload: Payload::new(text),
            ..lifted
        };

        // Run the full safety pipeline with Phase 2 no-op hooks.
        let awe = NoopAwe;
        let ack = NoopAck;
        let mut ctx = SafetyContext {
            capabilities: self.capabilities,
            awe_hook: &awe,
            ack_provider: &ack,
            audit_sink,
            anti_steg,
        };
        let report = validate(&envelope, self.dict, &mut ctx);

        ShadowEnvelopeResult {
            envelope: Some(envelope),
            audit_written: report.audit_written,
            status: format!("verdict={:?}", report.overall_verdict()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph_integration::sqlite_audit_sink::SqliteAuditSink;
    use glyph_core::embedded_v1_0_0;
    use glyph_safety::capability::AgentCapabilities;

    fn fully_authorized_registry(agent: &str) -> CapabilityRegistry {
        let dict = match embedded_v1_0_0() {
            Ok(d) => d,
            Err(e) => panic!("dict: {e}"),
        };
        let mut emits = Vec::new();
        for g in dict.glyphs() {
            emits.push((g.category.as_str().to_string(), g.char_repr.clone()));
        }
        let mut reg = CapabilityRegistry::new();
        reg.register(AgentCapabilities {
            agent_id: agent.to_string(),
            emits: emits.clone(),
            consumes: emits,
        });
        reg
    }

    #[test]
    fn shadow_envelope_writes_to_sink() {
        let dict = match embedded_v1_0_0() {
            Ok(d) => d,
            Err(e) => panic!("dict: {e}"),
        };
        let reg = fully_authorized_registry("test-agent");
        let builder = ShadowEnvelopeBuilder::new(&dict, &reg);

        let mut sink = match SqliteAuditSink::open_in_memory() {
            Ok(s) => s,
            Err(e) => panic!("sink: {e}"),
        };
        let mut anti = AntiStegMonitor::new();

        let result = builder.wrap_mcp_call(
            "test-agent",
            "analyze_content",
            "scored 12 items for relevance",
            &mut sink,
            &mut anti,
        );

        assert!(result.envelope.is_some());
        assert!(result.audit_written);
        assert_eq!(sink.len(), 1);
    }

    #[test]
    fn multiple_calls_accumulate_in_sink() {
        let dict = match embedded_v1_0_0() {
            Ok(d) => d,
            Err(e) => panic!("dict: {e}"),
        };
        let reg = fully_authorized_registry("repeat-agent");
        let builder = ShadowEnvelopeBuilder::new(&dict, &reg);

        let mut sink = match SqliteAuditSink::open_in_memory() {
            Ok(s) => s,
            Err(e) => panic!("sink: {e}"),
        };
        let mut anti = AntiStegMonitor::new();

        for i in 0..5 {
            builder.wrap_mcp_call(
                "repeat-agent",
                "get_relevant_content",
                &format!("returned {i} items"),
                &mut sink,
                &mut anti,
            );
        }
        assert_eq!(sink.len(), 5);
        let recent = sink.recent_for_agent("repeat-agent", 10);
        assert_eq!(recent.len(), 5);
    }
}

//! Phase 2 stubs for the AWE consequence-scan hook and the human-ack provider.
//!
//! In Phase 2 audit-only mode we don't block envelopes on AWE scans or
//! human acks — we just record them. These stubs satisfy the traits that
//! `glyph_safety::validate` expects without changing runtime behaviour.
//!
//! ## Phase 5 replacement plan
//!
//! When Phase 5 lands, replace these stubs with:
//!
//! - `AweMcpHook` — calls `mcp__awe__awe_consequence_scan` via the MCP
//!   bridge in `awe_commands.rs`, maps the response to
//!   `ReversibilityDecision` variants
//! - `WisdomGateAckProvider` — surfaces a modal prompt via the existing
//!   Wisdom Gate 2 UI in `src/components/wisdom*`, awaits the user's
//!   decision, returns `Granted`/`Denied`
//!
//! Both implementations live in THIS module — no changes to `glyph_safety`
//! itself. That crate is deliberately trait-based to keep AWE and UI
//! dependencies out of the safety library.

use glyph_core::{Dictionary, Envelope, EnvelopeId};
use glyph_safety::human_ack::{HumanAckProvider, HumanAckStatus};
use glyph_safety::reversibility::{AweConsequenceHook, ReversibilityDecision};

/// Phase 2 no-op AWE consequence-scan hook.
///
/// Always returns `NoScanNeeded`. The envelope is still routed through
/// the reversibility gate (the gate still fires for `⟲` and `🔒`), but
/// the scan itself is a no-op so downstream code sees `Propagate`.
pub struct NoopAwe;

impl AweConsequenceHook for NoopAwe {
    fn scan(&self, _env: &Envelope, _dict: &Dictionary) -> ReversibilityDecision {
        ReversibilityDecision::NoScanNeeded
    }
}

/// Phase 2 no-op human-ack provider.
///
/// Always returns `NotRequired`. Phase 5's `WisdomGateAckProvider` will
/// replace this with a real surface-to-UI implementation.
pub struct NoopAck;

impl HumanAckProvider for NoopAck {
    fn status(&self, _id: &EnvelopeId) -> HumanAckStatus {
        HumanAckStatus::NotRequired
    }
    fn request(&self, _env: &Envelope) {
        // Phase 5: surface a Wisdom Gate modal here.
    }
}

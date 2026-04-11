//! # glyph_integration — 4DA Phase 2 Audit-Only Mode
//!
//! This module bridges the [Glyph Envelope Protocol](https://github.com/runyourempire/glyph)
//! into 4DA's runtime. It is gated behind the `glyph_audit` Cargo feature
//! and is **audit-only** in Phase 2: every MCP tool call generates a
//! shadow envelope that is validated by `glyph-safety` and written to
//! the `glyph_audit` SQLite table. Agent behaviour is unchanged — we
//! are measuring, not routing.
//!
//! ## Architecture
//!
//! ```text
//!                                       MCP tool call
//!                                           │
//!                                           ▼
//!                                 ┌─────────────────┐
//!                                 │  mcp_envelope   │
//!                                 │  ::wrap_call    │
//!                                 └────────┬────────┘
//!                                          │
//!                                          ▼
//!                                 ┌─────────────────┐
//!                                 │  glyph_safety   │
//!                                 │  ::validate     │
//!                                 └────────┬────────┘
//!                                          │
//!               ┌──────────────────────────┼──────────────────────────┐
//!               ▼                          ▼                          ▼
//!       ┌──────────────┐           ┌──────────────┐           ┌──────────────┐
//!       │ wisdom_bridge│           │  wisdom_     │           │  sqlite_     │
//!       │ ::AweHook    │           │  ::AckProv.  │           │  audit_sink  │
//!       │ (Phase 2:    │           │ (Phase 2:    │           │              │
//!       │  NoScan)     │           │  NotReq'd)   │           │  glyph_audit │
//!       └──────────────┘           └──────────────┘           └──────────────┘
//! ```
//!
//! In Phase 5 the wisdom bridge stubs are replaced with real AWE and
//! Wisdom Gate UI bridges. In Phase 2 they are no-ops so audit-only mode
//! produces clean traffic data without any user-facing behaviour change.
//!
//! ## Phase 2 kill gates
//!
//! After one week of audit-only data:
//!
//! - Categorical coverage ≥ 50% of the 60-glyph alphabet must appear
//! - Audit log growth < 100MB/month at realistic rates
//! - `recent_for_agent` query 90th percentile < 10ms
//! - Zero anti-steg false positives across first 10k envelopes
//!
//! If any of these fail, Phase 2 is reverted by disabling the
//! `glyph_audit` feature. The code stays, the feature flag goes off,
//! zero user-visible impact.
//!
//! ## Activation
//!
//! ```bash
//! # Enable the feature (off by default)
//! pnpm tauri dev --config '{"build":{"features":["glyph_audit"]}}'
//!
//! # Or for builds:
//! cargo build --features glyph_audit --manifest-path src-tauri/Cargo.toml
//! ```
//!
//! ## On the compression story
//!
//! **GEP is not a compression technology.** Real Anthropic tokenizer
//! measurements (2026-04-12) showed plain English metadata beats the
//! best glyph form by more than 2× in token count. GEP earns its place
//! through visual distinctiveness in logs/dashboards, steganography
//! resistance via Gate 2, typed routing, composable safety gates, and
//! dual-form audit trails. Anyone quoting compression ratios is wrong.
//! See `docs/glyph/GEP-INTEGRATION.md` for the full honest verdict.

#![allow(dead_code)] // Phase 2 audit-only: some paths are wired but not yet
                    // routed from production code paths. They will be
                    // activated in Phase 3 when the first opt-in agent
                    // (gotcha-detector) begins emitting real envelopes.

pub mod mcp_envelope;
pub mod migration;
pub mod sqlite_audit_sink;
pub mod wisdom_bridge;

pub use mcp_envelope::{ShadowEnvelopeBuilder, ShadowEnvelopeResult};
pub use sqlite_audit_sink::SqliteAuditSink;
pub use wisdom_bridge::{NoopAwe, NoopAck};

/// Phase 2 audit-only mode sentinel. When this returns `true` the broker
/// writes shadow envelopes without changing routing behaviour. When
/// Phase 3+ lands the caller will check a richer state instead.
pub fn phase2_audit_only_enabled() -> bool {
    // Phase 2 default: always true when the feature is compiled in.
    true
}

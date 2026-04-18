// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Runtime schema validation for EvidenceItem.
//!
//! In debug builds, `validate_item` panics with diagnostic on violation —
//! bugs surface immediately during development.
//! In release builds, `validate_item` returns `Err(ValidationError)` — the
//! caller is expected to drop the item and emit a structured log.
//!
//! Rules come from `docs/strategy/EVIDENCE-ITEM-SCHEMA.md`.

use super::types::{Action, ConfidenceProvenance, EvidenceItem, EvidenceKind, ACTION_IDS};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ValidationError {
    #[error("title empty")]
    TitleEmpty,
    #[error("title too long: {len} (max 120)")]
    TitleTooLong { len: usize },
    #[error("title ends with period — per schema no trailing period")]
    TitleTrailingPeriod,
    #[error("confidence value out of range: {0}")]
    ConfidenceOutOfRange(f32),
    #[error("Calibrated confidence missing sample_size")]
    CalibratedMissingN,
    #[error("Calibrated confidence N too small: {n} (min 10)")]
    CalibratedNTooSmall { n: u32 },
    #[error("reversibility out of range: {0}")]
    ReversibilityOutOfRange(f32),
    #[error("user-surfaced kind {kind:?} requires non-empty evidence")]
    EvidenceRequired { kind: EvidenceKind },
    #[error("actionable kind {kind:?} requires ≥1 suggested action")]
    ActionsRequired { kind: EvidenceKind },
    #[error("unknown action_id: {0}")]
    UnknownActionId(String),
    #[error("citation relevance_note too long: {len} (max 200)")]
    CitationNoteTooLong { len: usize },
    #[error("precedent similarity out of range: {0}")]
    PrecedentSimilarityOutOfRange(f32),
    #[error("id is empty")]
    IdEmpty,
    #[error("explanation required after AWE spine wired (Phase 9+)")]
    #[allow(dead_code)] // Gated on a future phase flag; checker wired in Phase 9.
    ExplanationRequired,
}

/// Kinds that are user-surfaced and require at least one citation.
fn requires_evidence(kind: EvidenceKind) -> bool {
    !matches!(kind, EvidenceKind::Retrospective)
}

/// Kinds that are actionable and require ≥1 suggested action.
fn requires_actions(kind: EvidenceKind) -> bool {
    matches!(
        kind,
        EvidenceKind::Alert | EvidenceKind::Gap | EvidenceKind::Decision | EvidenceKind::Refutation
    )
}

fn validate_action(a: &Action) -> Result<(), ValidationError> {
    if !ACTION_IDS.iter().any(|id| *id == a.action_id) {
        return Err(ValidationError::UnknownActionId(a.action_id.clone()));
    }
    Ok(())
}

/// Validate an `EvidenceItem` against the canonical schema.
///
/// Call this at the output boundary of every materializer. In debug builds
/// a violation should panic at the call site; in release the caller should
/// drop the item with a structured log.
pub fn validate_item(item: &EvidenceItem) -> Result<(), ValidationError> {
    if item.id.is_empty() {
        return Err(ValidationError::IdEmpty);
    }
    // Title rules
    if item.title.is_empty() {
        return Err(ValidationError::TitleEmpty);
    }
    if item.title.len() > 120 {
        return Err(ValidationError::TitleTooLong {
            len: item.title.len(),
        });
    }
    if item.title.ends_with('.') {
        return Err(ValidationError::TitleTrailingPeriod);
    }

    // Confidence rules
    let c = &item.confidence;
    if !(0.0..=1.0).contains(&c.value) {
        return Err(ValidationError::ConfidenceOutOfRange(c.value));
    }
    if c.provenance == ConfidenceProvenance::Calibrated {
        match c.sample_size {
            None => return Err(ValidationError::CalibratedMissingN),
            Some(n) if n < 10 => return Err(ValidationError::CalibratedNTooSmall { n }),
            _ => {}
        }
    }

    // Reversibility rules
    if let Some(r) = item.reversibility {
        if !(0.0..=1.0).contains(&r) {
            return Err(ValidationError::ReversibilityOutOfRange(r));
        }
    }

    // Evidence rules
    if requires_evidence(item.kind) && item.evidence.is_empty() {
        return Err(ValidationError::EvidenceRequired { kind: item.kind });
    }
    for cite in &item.evidence {
        if cite.relevance_note.len() > 200 {
            return Err(ValidationError::CitationNoteTooLong {
                len: cite.relevance_note.len(),
            });
        }
    }

    // Action rules
    if requires_actions(item.kind) && item.suggested_actions.is_empty() {
        return Err(ValidationError::ActionsRequired { kind: item.kind });
    }
    for a in &item.suggested_actions {
        validate_action(a)?;
    }

    // Precedent rules
    for p in &item.precedents {
        if !(0.0..=1.0).contains(&p.similarity) {
            return Err(ValidationError::PrecedentSimilarityOutOfRange(p.similarity));
        }
    }

    Ok(())
}

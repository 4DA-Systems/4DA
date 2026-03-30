// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * AWE (Artificial Wisdom Engine) TypeScript types.
 *
 * These types mirror the structured JSON responses from AWE Tauri commands.
 * Each type is designed for a specific page's AWE integration.
 */

// ---------------------------------------------------------------------------
// Summary (used across pages)
// ---------------------------------------------------------------------------

export interface AweSummary {
  available: boolean;
  decisions: number;
  principles: number;
  pending: number;
  feedback_count: number;
  feedback_coverage: number;
  top_principle: string | null;
  health: 'healthy' | 'good' | 'learning' | 'needs_feedback' | 'cold' | null;
}

// ---------------------------------------------------------------------------
// Pattern Matching (Briefing page)
// ---------------------------------------------------------------------------

export interface AwePatternMatch {
  precedents: AwePrecedent[];
  principles: AwePrincipleRef[];
  anti_patterns: AweAntiPatternRef[];
}

export interface AwePrecedent {
  description: string;
  outcome: string;
  relevance: number;
}

export interface AwePrincipleRef {
  statement: string;
  confidence: number;
  evidence_count: number;
}

export interface AweAntiPatternRef {
  pattern: string;
  failure_mode: string;
  confidence: number;
}

// ---------------------------------------------------------------------------
// Decision History (Results page)
// ---------------------------------------------------------------------------

export interface AweDecisionSummary {
  id: string;
  statement: string;
  action: string;
  confidence: number;
  domain: string;
  timestamp: string;
  has_feedback: boolean;
  outcome: 'confirmed' | 'refuted' | 'partial' | 'pending' | null;
}

// ---------------------------------------------------------------------------
// Pending Decisions (Console Feedback Queue)
// ---------------------------------------------------------------------------

export interface AwePendingDecision {
  id: string;
  statement: string;
  domain: string;
  timestamp: string;
  age_days: number;
}

// ---------------------------------------------------------------------------
// Wisdom Well (Console depth visualization)
// ---------------------------------------------------------------------------

export interface AweWisdomWell {
  surface: AweWellItem[];
  pattern: AweWellItem[];
  principle: AweWellItem[];
  causal: AweWellItem[];
  meta: AweWellItem[];
  universal: AweWellItem[];
}

export interface AweWellItem {
  statement: string;
  confidence: number;
  evidence_count: number;
}

// ---------------------------------------------------------------------------
// Growth Trajectory (Profile page)
// ---------------------------------------------------------------------------

export interface AweGrowthTrajectory {
  decisions: number;
  feedback_count: number;
  feedback_coverage: number;
  principles_formed: number;
  anti_patterns_detected: number;
  growth_phase: 'cold_start' | 'accumulating' | 'compounding' | 'mature';
}

// ---------------------------------------------------------------------------
// Batch Feedback Response
// ---------------------------------------------------------------------------

export interface AweBatchFeedbackResult {
  submitted: number;
  errors: number;
  total: number;
}

// ---------------------------------------------------------------------------
// Auto-Feedback Response
// ---------------------------------------------------------------------------

export interface AweAutoFeedbackResult {
  decisions_stored: number;
  outcomes_inferred: number;
  repos_scanned: number;
}

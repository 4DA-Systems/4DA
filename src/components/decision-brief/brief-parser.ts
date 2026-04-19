// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Brief Parser — Intelligence Reconciliation Phase 10 (2026-04-17).
 *
 * Converts AWE's `run_awe_transmute` output string (a JSON string) into
 * the canonical `DecisionBriefData` shape rendered by DecisionBrief.
 *
 * AWE returns fields like `wisdom`, `confidence`, `watch_for`, `mode`
 * (see context_commands.rs). This parser extracts and normalizes them,
 * filling in conservative defaults when fields are missing so the UI
 * never crashes on a partial response.
 */

import type { DecisionBriefData, BriefPrecedent } from './DecisionBrief';

// ============================================================================
// Types for raw AWE output (best-effort — AWE's JSON is semi-structured)
// ============================================================================

interface RawBrief {
  wisdom?: string;
  confidence?: number;
  watch_for?: string[];
  mode?: string;
  assumptions?: string[];
  reversibility?: number;
  worst_case?: string;
  precedents?: Array<{
    statement?: string;
    outcome?: string;
    origin?: string;
    similarity?: number;
  }>;
  verdict?: string;
  confidence_provenance?: string;
}

// ============================================================================
// Parsing
// ============================================================================

/**
 * Parse the raw transmute result string into `DecisionBriefData`.
 * Conservative on every field: missing data becomes a safe default,
 * never a runtime crash.
 *
 * @param raw - the result of `cmd('run_awe_transmute', …)`, which is
 *   a JSON string produced by the Rust command handler.
 * @param originalQuery - the user's original decision sentence; used
 *   as the fallback for `decision` when AWE didn't restate it.
 */
export function parseBrief(
  raw: string,
  originalQuery: string,
): DecisionBriefData {
  let parsed: RawBrief = {};
  try {
    parsed = JSON.parse(raw) as RawBrief;
  } catch {
    // AWE can return plain text on error (e.g. "AWE binary not found").
    // Treat that as the verdict and surface it honestly.
    return {
      decision: originalQuery,
      assumptions: [],
      reversibility: undefined,
      worstCase: undefined,
      precedents: [],
      verdict: raw.slice(0, 200),
      confidence: 0,
      confidenceProvenance: 'heuristic',
      mode: 'structured',
    };
  }

  const assumptions = normalizeStringArray(
    parsed.assumptions ?? parsed.watch_for ?? [],
  ).slice(0, 3);

  const precedents = normalizePrecedents(parsed.precedents ?? []).slice(0, 3);

  return {
    decision: typeof parsed.wisdom === 'string' && parsed.wisdom.length > 0
      ? firstSentence(parsed.wisdom)
      : originalQuery,
    assumptions,
    reversibility: clampUnit(parsed.reversibility),
    worstCase: typeof parsed.worst_case === 'string' ? parsed.worst_case : undefined,
    precedents,
    verdict: typeof parsed.verdict === 'string' && parsed.verdict.length > 0
      ? parsed.verdict
      : typeof parsed.wisdom === 'string'
        ? parsed.wisdom
        : '—',
    confidence: clampUnit(parsed.confidence) ?? 0.5,
    confidenceProvenance: typeof parsed.confidence_provenance === 'string'
      ? parsed.confidence_provenance
      : 'heuristic',
    mode: typeof parsed.mode === 'string' ? parsed.mode : 'structured',
  };
}

// ============================================================================
// Helpers — pure, testable
// ============================================================================

function normalizeStringArray(input: unknown): string[] {
  if (!Array.isArray(input)) return [];
  return input
    .filter((x): x is string => typeof x === 'string' && x.trim().length > 0)
    .map(s => s.trim());
}

function normalizePrecedents(input: unknown): BriefPrecedent[] {
  if (!Array.isArray(input)) return [];
  const out: BriefPrecedent[] = [];
  for (const item of input) {
    if (typeof item !== 'object' || item === null) continue;
    const rec = item as Record<string, unknown>;
    const statement = typeof rec.statement === 'string' ? rec.statement : '';
    if (statement.length === 0) continue;
    const outcome = normalizeOutcome(rec.outcome);
    const origin = typeof rec.origin === 'string' ? rec.origin : 'unknown';
    const similarity = clampUnit(rec.similarity) ?? 0;
    out.push({ statement, outcome, origin, similarity });
  }
  return out;
}

function normalizeOutcome(raw: unknown): BriefPrecedent['outcome'] {
  if (typeof raw !== 'string') return 'pending';
  switch (raw.toLowerCase()) {
    case 'confirmed':
      return 'confirmed';
    case 'refuted':
      return 'refuted';
    case 'partial':
      return 'partial';
    case 'pending':
    default:
      return 'pending';
  }
}

function clampUnit(input: unknown): number | undefined {
  if (typeof input !== 'number') return undefined;
  if (!Number.isFinite(input)) return undefined;
  return Math.max(0, Math.min(1, input));
}

/** First sentence, capped at 200 chars. Used to make AWE's wisdom
 * string fit the one-line "decision" slot of the brief. The cap
 * includes the ellipsis, so the slice is 199 + "…" = 200 chars. */
export function firstSentence(text: string): string {
  const trimmed = text.trim();
  const match = trimmed.match(/^[^.!?]+[.!?]?/);
  const sentence = match ? match[0].trim() : trimmed;
  return sentence.length > 200 ? `${sentence.slice(0, 199)}…` : sentence;
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { describe, it, expect } from 'vitest';
import { parseBrief, firstSentence } from './brief-parser';

describe('brief-parser', () => {
  describe('parseBrief', () => {
    it('parses a full AWE response', () => {
      const raw = JSON.stringify({
        wisdom: 'Adopting Turborepo is reasonable for your stack.',
        confidence: 0.72,
        watch_for: [
          'Build time regression',
          'Team familiarity',
          'pnpm workspace compatibility',
        ],
        mode: 'structured',
        reversibility: 0.3,
        worst_case: 'Revert to pnpm workspaces, 1 day of work.',
        precedents: [
          {
            statement: 'Adopted pnpm workspaces',
            outcome: 'confirmed',
            origin: 'user-history',
            similarity: 0.65,
          },
        ],
        verdict: 'Lean toward adopt.',
        confidence_provenance: 'llm_assessed',
      });
      const brief = parseBrief(raw, 'Should I adopt Turborepo?');
      expect(brief.decision).toBe('Adopting Turborepo is reasonable for your stack.');
      expect(brief.confidence).toBeCloseTo(0.72);
      expect(brief.assumptions).toHaveLength(3);
      expect(brief.reversibility).toBeCloseTo(0.3);
      expect(brief.worstCase).toContain('Revert to pnpm workspaces');
      expect(brief.precedents).toHaveLength(1);
      expect(brief.precedents[0]!.outcome).toBe('confirmed');
      expect(brief.verdict).toBe('Lean toward adopt.');
      expect(brief.confidenceProvenance).toBe('llm_assessed');
      expect(brief.mode).toBe('structured');
    });

    it('falls back to watch_for when assumptions missing', () => {
      const raw = JSON.stringify({
        wisdom: 'x',
        watch_for: ['a', 'b'],
      });
      const brief = parseBrief(raw, 'q');
      expect(brief.assumptions).toEqual(['a', 'b']);
    });

    it('caps assumptions at 3', () => {
      const raw = JSON.stringify({
        wisdom: 'x',
        watch_for: ['1', '2', '3', '4', '5'],
      });
      const brief = parseBrief(raw, 'q');
      expect(brief.assumptions).toHaveLength(3);
    });

    it('caps precedents at 3', () => {
      const raw = JSON.stringify({
        wisdom: 'x',
        precedents: Array.from({ length: 10 }, (_, i) => ({
          statement: `p${i}`,
          outcome: 'pending',
          origin: 'curated-corpus',
          similarity: 0.5,
        })),
      });
      const brief = parseBrief(raw, 'q');
      expect(brief.precedents).toHaveLength(3);
    });

    it('uses originalQuery when wisdom is missing', () => {
      const raw = JSON.stringify({ confidence: 0.5 });
      const brief = parseBrief(raw, 'My original sentence.');
      expect(brief.decision).toBe('My original sentence.');
    });

    it('defaults confidence to 0.5 when absent', () => {
      const raw = JSON.stringify({ wisdom: 'ok' });
      const brief = parseBrief(raw, 'q');
      expect(brief.confidence).toBe(0.5);
    });

    it('clamps confidence into [0, 1]', () => {
      const over = parseBrief(JSON.stringify({ wisdom: 'x', confidence: 1.5 }), 'q');
      const under = parseBrief(JSON.stringify({ wisdom: 'x', confidence: -0.2 }), 'q');
      expect(over.confidence).toBe(1);
      expect(under.confidence).toBe(0);
    });

    it('treats non-JSON response as an error verdict', () => {
      const brief = parseBrief('AWE binary not found.', 'q');
      expect(brief.verdict).toContain('AWE binary not found');
      expect(brief.confidence).toBe(0);
    });

    it('normalizes unknown outcome strings to pending', () => {
      const raw = JSON.stringify({
        wisdom: 'x',
        precedents: [
          { statement: 'p1', outcome: 'mystery', origin: 'curated-corpus', similarity: 0.5 },
          { statement: 'p2', outcome: 'CONFIRMED', origin: 'curated-corpus', similarity: 0.5 },
        ],
      });
      const brief = parseBrief(raw, 'q');
      expect(brief.precedents[0]!.outcome).toBe('pending');
      expect(brief.precedents[1]!.outcome).toBe('confirmed');
    });

    it('drops precedents without a statement', () => {
      const raw = JSON.stringify({
        wisdom: 'x',
        precedents: [
          { outcome: 'confirmed', origin: 'x', similarity: 0.5 },
          { statement: 'ok', outcome: 'confirmed', origin: 'x', similarity: 0.5 },
        ],
      });
      const brief = parseBrief(raw, 'q');
      expect(brief.precedents).toHaveLength(1);
      expect(brief.precedents[0]!.statement).toBe('ok');
    });

    it('defaults confidenceProvenance to heuristic', () => {
      const brief = parseBrief(JSON.stringify({ wisdom: 'x' }), 'q');
      expect(brief.confidenceProvenance).toBe('heuristic');
    });
  });

  describe('firstSentence', () => {
    it('extracts up to the first terminator', () => {
      expect(firstSentence('Adopt it. Reconsider later.')).toBe('Adopt it.');
      expect(firstSentence('Is this right? Yes.')).toBe('Is this right?');
    });

    it('handles a missing terminator', () => {
      expect(firstSentence('Adopt it confidently')).toBe('Adopt it confidently');
    });

    it('caps at 200 chars with ellipsis', () => {
      const s = firstSentence('a'.repeat(500));
      expect(s.endsWith('…')).toBe(true);
      expect(s.length).toBe(200);
    });

    it('trims whitespace', () => {
      expect(firstSentence('   leading and trailing.   ')).toBe('leading and trailing.');
    });
  });
});

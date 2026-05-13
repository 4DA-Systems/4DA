// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import {
  getScoreTier, extractItemId, depFromItem, signalMatchesDep, sourceTypeLabel,
  URGENCY_ORDER,
} from '../types';
import type { EvidenceItem } from '../../../../src-tauri/bindings/bindings/EvidenceItem';

function makeItem(overrides: Partial<EvidenceItem>): EvidenceItem {
  return {
    id: 'test-0',
    kind: 'MissedSignal',
    title: 'Test item',
    explanation: 'Test explanation',
    confidence: { level: 0.5, provenance: 'Heuristic' },
    urgency: 'medium',
    reversibility: null,
    evidence: [],
    affected_projects: [],
    affected_deps: [],
    suggested_actions: [],
    precedents: [],
    refutation_condition: null,
    lens_hints: { lenses: ['blind_spots'] },
    created_at: Date.now(),
    expires_at: null,
    ...overrides,
  } as EvidenceItem;
}

describe('getScoreTier', () => {
  it('returns green for low scores (good coverage)', () => {
    const tier = getScoreTier(0);
    expect(tier.color).toContain('green');
  });

  it('returns green for score 25', () => {
    const tier = getScoreTier(25);
    expect(tier.color).toContain('green');
  });

  it('returns yellow for moderate gaps (26-50)', () => {
    const tier = getScoreTier(30);
    expect(tier.color).toContain('yellow');
  });

  it('returns orange for significant gaps (51-75)', () => {
    const tier = getScoreTier(60);
    expect(tier.color).toContain('orange');
  });

  it('returns red for critical blind spots (76-100)', () => {
    const tier = getScoreTier(90);
    expect(tier.color).toContain('red');
  });
});

describe('extractItemId', () => {
  it('extracts numeric ID from bs_missed_ prefix', () => {
    expect(extractItemId('bs_missed_42')).toBe(42);
  });

  it('extracts numeric ID from llm-bs- prefix', () => {
    expect(extractItemId('llm-bs-123')).toBe(123);
  });

  it('returns null for non-matching IDs', () => {
    expect(extractItemId('bs_uncov_npm_react')).toBeNull();
    expect(extractItemId('bs_stale_rust')).toBeNull();
    expect(extractItemId('bs_rec_0')).toBeNull();
  });
});

describe('depFromItem', () => {
  it('returns first dep when available', () => {
    const item = makeItem({ affected_deps: ['react', 'react-dom'] });
    expect(depFromItem(item)).toBe('react');
  });

  it('returns null when no deps', () => {
    const item = makeItem({ affected_deps: [] });
    expect(depFromItem(item)).toBeNull();
  });
});

describe('signalMatchesDep', () => {
  it('matches by affected_deps', () => {
    const signal = makeItem({ affected_deps: ['tokio'] });
    expect(signalMatchesDep(signal, 'tokio')).toBe(true);
    expect(signalMatchesDep(signal, 'Tokio')).toBe(true);
  });

  it('matches by title substring', () => {
    const signal = makeItem({ title: 'New tokio release v1.40', affected_deps: [] });
    expect(signalMatchesDep(signal, 'tokio')).toBe(true);
  });

  it('does not match unrelated deps', () => {
    const signal = makeItem({ title: 'React update', affected_deps: ['react'] });
    expect(signalMatchesDep(signal, 'vue')).toBe(false);
  });
});

describe('sourceTypeLabel', () => {
  it('returns release for registry sources', () => {
    expect(sourceTypeLabel('npm_registry')?.label).toBe('release');
    expect(sourceTypeLabel('crates_io')?.label).toBe('release');
  });

  it('returns article for devto', () => {
    expect(sourceTypeLabel('devto')?.label).toBe('article');
  });

  it('returns null for unknown sources', () => {
    expect(sourceTypeLabel('unknown_source')).toBeNull();
  });
});

describe('URGENCY_ORDER', () => {
  it('sorts critical before high before medium before watch', () => {
    expect(URGENCY_ORDER.critical).toBeLessThan(URGENCY_ORDER.high);
    expect(URGENCY_ORDER.high).toBeLessThan(URGENCY_ORDER.medium);
    expect(URGENCY_ORDER.medium).toBeLessThan(URGENCY_ORDER.watch);
  });
});

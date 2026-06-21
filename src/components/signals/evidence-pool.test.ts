// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { computeEvidencePool, isGrounded, groundingDeps } from './evidence-pool';
import type { SourceRelevance } from '../../types';

function item(partial: Partial<SourceRelevance>): SourceRelevance {
  return {
    id: 1,
    title: 't',
    url: null,
    top_score: 0.9,
    matches: [],
    relevant: true,
    ...partial,
  };
}

describe('computeEvidencePool', () => {
  it('routes a dependency-matched item to Affects You', () => {
    const r = item({ score_breakdown: { matched_deps: ['react'] } as never });
    expect(isGrounded(r)).toBe(true);
    expect(computeEvidencePool(r)).toBe('affects_you');
    expect(groundingDeps(r)).toEqual(['react']);
  });

  it('routes a confirmed advisory (is_critical_alert) to Affects You even with no matched_deps', () => {
    const r = item({ is_critical_alert: true });
    expect(computeEvidencePool(r)).toBe('affects_you');
  });

  it('routes an affected/likely_affected applicability to Affects You', () => {
    expect(computeEvidencePool(item({ applicability: 'affected' }))).toBe('affects_you');
    expect(computeEvidencePool(item({ applicability: 'likely_affected' }))).toBe('affects_you');
  });

  it('routes a high domain-relevance but ungrounded item to In Your Orbit', () => {
    const r = item({ score_breakdown: { domain_relevance: 0.85, matched_deps: [] } as never });
    expect(isGrounded(r)).toBe(false);
    expect(computeEvidencePool(r)).toBe('in_orbit');
  });

  it('routes a high-score but off-domain, ungrounded item to Ambient', () => {
    // The live failure case: 0.91 relevance, no stack tie, low domain relevance.
    const r = item({ top_score: 0.91, score_breakdown: { domain_relevance: 0.15, matched_deps: [] } as never });
    expect(computeEvidencePool(r)).toBe('ambient');
  });

  it('treats not_affected applicability as ungrounded', () => {
    const r = item({ applicability: 'not_affected', score_breakdown: { domain_relevance: 0.2 } as never });
    expect(computeEvidencePool(r)).toBe('ambient');
  });

  it('defaults a bare item (no breakdown) to Ambient', () => {
    expect(computeEvidencePool(item({}))).toBe('ambient');
    expect(groundingDeps(item({}))).toEqual([]);
  });
});

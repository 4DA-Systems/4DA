// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Tests for useBlindSpotsData — the lane merge that shapes the Blind Spots
 * view. Pins the two 2026-08-31 live-audit defects:
 *
 * 1. A missed signal carrying a bare dep name ("react") must land on the
 *    ecosystem-qualified gap row ("react (npm)") instead of minting a second
 *    bare row beside it.
 * 2. The Emerging list must not show the same story twice because two sources
 *    (mastodon + hackernews) carried the same URL under different titles.
 */
import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';

import { useBlindSpotsData } from '../use-blind-spots-data';
import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

function makeEvidenceItem(overrides: Partial<EvidenceItem> & { id: string }): EvidenceItem {
  return {
    kind: 'missed_signal',
    title: 'Some signal title',
    explanation: '',
    confidence: { value: 0.6, provenance: 'heuristic' as never, sample_size: null },
    urgency: 'medium',
    reversibility: null,
    evidence: [],
    affected_projects: [],
    affected_deps: [],
    suggested_actions: [],
    precedents: [],
    refutation_condition: null,
    lens_hints: {
      briefing: false, preemption: false, blind_spots: true, evidence: false,
      other_build_target: false, upgrade_plan: false,
    },
    created_at: BigInt(0),
    expires_at: null,
    ...overrides,
  };
}

function makeCitation(url: string | null) {
  return {
    source: 'hackernews',
    title: 'citation',
    url,
    freshness_days: 4,
    relevance_note: '',
  };
}

function makeFeed(items: EvidenceItem[]): EvidenceFeed {
  return { items, total: items.length, critical_count: 0, high_count: 0, score: 10 };
}

describe('useBlindSpotsData', () => {
  it('merges a bare-dep missed signal into the qualified dependency row', () => {
    const feed = makeFeed([
      makeEvidenceItem({
        id: 'bs_uncov_npm_react (npm)',
        kind: 'gap',
        title: 'react (npm) — 3 updates to review',
        affected_deps: ['react (npm)'],
      }),
      // Three signals naming the bare dep, none of whose titles contain
      // the literal string "react (npm)".
      makeEvidenceItem({ id: 'bs_missed_1', title: 'A hooks deep dive', affected_deps: ['react'] }),
      makeEvidenceItem({ id: 'bs_missed_2', title: 'Concurrent rendering notes', affected_deps: ['react'] }),
      makeEvidenceItem({ id: 'bs_missed_3', title: 'Compiler release explained', affected_deps: ['react'] }),
    ]);

    const { result } = renderHook(() => useBlindSpotsData(feed, new Set()));

    // ONE row, not a "react (npm)" row plus a bare "react" shadow.
    expect(result.current.depRows).toHaveLength(1);
    expect(result.current.depRows[0]!.name).toBe('react (npm)');
    expect(result.current.depRows[0]!.signals).toHaveLength(3);
    expect(result.current.unmatchedSignals).toHaveLength(0);
  });

  it('still creates a row for a dep with signals but no gap entry', () => {
    const feed = makeFeed([
      makeEvidenceItem({ id: 'bs_missed_1', title: 'Axum 0.9 middleware guide', affected_deps: ['axum'] }),
    ]);
    const { result } = renderHook(() => useBlindSpotsData(feed, new Set()));
    expect(result.current.depRows).toHaveLength(1);
    expect(result.current.depRows[0]!.name).toBe('axum');
  });

  it('collapses same-URL missed signals in the Emerging list', () => {
    // The arrayref shape: the same safedep.io story once via mastodon and
    // once via hackernews — different titles, one URL.
    const feed = makeFeed([
      makeEvidenceItem({
        id: 'bs_missed_10',
        title: 'Malicious arrayref clone hits crates.io',
        evidence: [makeCitation('https://safedep.io/arrayref-supply-chain/')],
      }),
      makeEvidenceItem({
        id: 'bs_missed_11',
        title: 'Supply chain attack on Rust arrayref — analysis',
        evidence: [makeCitation('https://www.safedep.io/arrayref-supply-chain?utm_source=hn')],
      }),
      makeEvidenceItem({
        id: 'bs_missed_12',
        title: 'A different story entirely',
        evidence: [makeCitation('https://example.com/other')],
      }),
    ]);

    const { result } = renderHook(() => useBlindSpotsData(feed, new Set()));

    // First (highest-ranked) copy survives; the same-URL retelling is gone.
    const titles = result.current.unmatchedSignals.map(s => s.title);
    expect(titles).toContain('Malicious arrayref clone hits crates.io');
    expect(titles).not.toContain('Supply chain attack on Rust arrayref — analysis');
    expect(titles).toContain('A different story entirely');
  });

  it('keeps signals without URLs intact', () => {
    const feed = makeFeed([
      makeEvidenceItem({ id: 'bs_missed_20', title: 'No URL A', evidence: [makeCitation(null)] }),
      makeEvidenceItem({ id: 'bs_missed_21', title: 'No URL B', evidence: [] }),
    ]);
    const { result } = renderHook(() => useBlindSpotsData(feed, new Set()));
    expect(result.current.unmatchedSignals).toHaveLength(2);
  });
});

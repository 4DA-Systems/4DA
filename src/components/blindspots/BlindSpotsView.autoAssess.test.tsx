// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';
import type { DepRow } from './types';

// Auto-assess: when the `auto_assess_blind_spots` setting is on AND a cloud LLM
// key is present, the Blind Spots lens runs the AI triage automatically on a
// dep-set change — no click. It must NOT fire when the toggle is off or when no
// key is configured (local-only / key-less users keep the manual button).

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../../lib/trust-feedback', () => ({ recordTrustEvent: vi.fn() }));
vi.mock('./dismissal-utils', () => ({
  loadPersistedDismissals: () => new Set<string>(),
  persistDismissal: vi.fn(),
  removeDismissal: vi.fn(),
}));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./StackCoverageMap', () => ({ TierSection: () => null, EmergingSignals: () => null }));
vi.mock('./CollapsedSections', () => ({
  CoveredSection: () => null,
  NoCoverageSection: () => null,
  OtherBuildTargetsSection: () => null,
  ProbablyFineSection: () => null,
}));

const assessment = {
  assessments: [{ dep_name: 'react (npm)', worth_reviewing: true, recommendation: 'x' }],
  model: 'claude-sonnet-4-6',
  assessed_at: 0,
  from_cache: false,
};

let cachedAssessment: unknown = null;
const cmdMock = vi.fn((name: string) => {
  if (name === 'assess_blind_spots_with_ai') return Promise.resolve(assessment);
  if (name === 'get_cached_blind_spot_assessment') return Promise.resolve(cachedAssessment);
  return Promise.resolve(null);
});
vi.mock('../../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string])) }));

let mockDepRows: DepRow[] = [];
vi.mock('../../hooks/use-blind-spots-data', () => ({
  useBlindSpotsData: () => ({ depRows: mockDepRows, unmatchedSignals: [], recommendations: [] }),
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function depRow(name: string): DepRow {
  return {
    name, status: 'blind_spot', urgency: 'high',
    gap: { id: `bs_uncov_x_${name}`, affected_deps: [name], lens_hints: { other_build_target: false, upgrade_plan: false } } as unknown as DepRow['gap'],
    signals: [], projects: [],
  };
}

function baseState(settings: unknown): Record<string, unknown> {
  return {
    blindSpotReport: { items: [], score: 50, total_tracked: 1, weak_match_count: 0, data_freshness: null },
    blindSpotsLoading: false, blindSpotsError: null, blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(),
    settings,
  };
}

const assessCalls = () => cmdMock.mock.calls.filter(c => c[0] === 'assess_blind_spots_with_ai');

beforeEach(() => {
  cmdMock.mockClear();
  cachedAssessment = null;
  mockDepRows = [depRow('react (npm)')];
});

describe('BlindSpotsView — auto-assess on dep-set change', () => {
  it('auto-runs the triage when the toggle is on and a cloud key is present (no click)', async () => {
    mockState = baseState({ auto_assess_blind_spots: true, llm: { has_api_key: true } });
    render(<BlindSpotsView />);
    await waitFor(() => expect(assessCalls().length).toBeGreaterThan(0));
  });

  it('does NOT auto-run when the toggle is off', async () => {
    mockState = baseState({ auto_assess_blind_spots: false, llm: { has_api_key: true } });
    render(<BlindSpotsView />);
    // give effects a tick; get_cached/source_health may fire, assess must not
    await new Promise(r => setTimeout(r, 50));
    expect(assessCalls()).toHaveLength(0);
  });

  it('does NOT auto-run when no cloud LLM key is configured', async () => {
    mockState = baseState({ auto_assess_blind_spots: true, llm: { has_api_key: false } });
    render(<BlindSpotsView />);
    await new Promise(r => setTimeout(r, 50));
    expect(assessCalls()).toHaveLength(0);
  });

  it('does NOT auto-run when there are no surfaced gap deps', async () => {
    mockDepRows = []; // nothing surfaced
    mockState = baseState({ auto_assess_blind_spots: true, llm: { has_api_key: true } });
    render(<BlindSpotsView />);
    await new Promise(r => setTimeout(r, 50));
    expect(assessCalls()).toHaveLength(0);
  });

  // Audit 2026-10-07: the last-assessed set lived in a ref that was empty on
  // every mount, so every tab open re-ran the model (19 Sonnet calls). The
  // persisted verdict is read first; only a stale/missing one auto-assesses.
  it('does NOT re-run on mount when the persisted verdict is current', async () => {
    cachedAssessment = { ...assessment, from_cache: true, stale: false };
    mockState = baseState({ auto_assess_blind_spots: true, llm: { has_api_key: true } });
    render(<BlindSpotsView />);
    await waitFor(() => expect(cmdMock).toHaveBeenCalledWith('get_cached_blind_spot_assessment'));
    await new Promise(r => setTimeout(r, 50));
    expect(assessCalls()).toHaveLength(0);
  });

  it('re-assesses (without force) when the persisted verdict is stale', async () => {
    cachedAssessment = { ...assessment, from_cache: true, stale: true };
    mockState = baseState({ auto_assess_blind_spots: true, llm: { has_api_key: true } });
    render(<BlindSpotsView />);
    await waitFor(() => expect(assessCalls()).toHaveLength(1));
    expect(assessCalls()[0]).toEqual(['assess_blind_spots_with_ai', { force: false }]);
  });

  it('auto-runs at most once per mount even as the report refreshes', async () => {
    mockState = baseState({ auto_assess_blind_spots: true, llm: { has_api_key: true } });
    const { rerender } = render(<BlindSpotsView />);
    await waitFor(() => expect(assessCalls()).toHaveLength(1));
    mockState = { ...mockState, blindSpotReport: { items: [], score: 40, total_tracked: 1, weak_match_count: 0, data_freshness: null } };
    rerender(<BlindSpotsView />);
    await new Promise(r => setTimeout(r, 50));
    expect(assessCalls()).toHaveLength(1);
  });
});

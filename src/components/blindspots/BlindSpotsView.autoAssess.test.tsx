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
vi.mock('./ScoreBar', () => ({ default: () => <div /> }));
vi.mock('./StackCoverageMap', () => ({ TierSection: () => null, EmergingSignals: () => null }));
vi.mock('./CollapsedSections', () => ({
  CoveredSection: () => null,
  OtherBuildTargetsSection: () => null,
  ProbablyFineSection: () => null,
}));

const assessment = {
  assessments: [{ dep_name: 'react (npm)', worth_reviewing: true, recommendation: 'x' }],
  model: 'claude-sonnet-4-6',
  assessed_at: 0,
  from_cache: false,
};

const cmdMock = vi.fn((name: string) => {
  if (name === 'assess_blind_spots_with_ai') return Promise.resolve(assessment);
  if (name === 'get_source_health') return Promise.resolve({ total_active: 1, total_failing: 0, total_disabled: 0 });
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
    gap: { id: `bs_uncov_x_${name}`, affected_deps: [name], lens_hints: { other_build_target: false } } as unknown as DepRow['gap'],
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
});

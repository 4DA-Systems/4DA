// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';
import type { DepRow } from './types';

// Audit 2026-10-07 (fresh-profile E2E): day-one Blind Spots showed "Building
// your coverage picture… Check back soon" — a banned empty state (doctrine
// rule 6) — above a "27/100 Moderate gaps" score that informs no action
// (rule 3), and a "N source adapters failing" banner unrelated to this tab.

let coldStart = false;
vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => coldStart }));
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

const cmdMock = vi.fn((name: string) => {
  if (name === 'get_source_health') return Promise.resolve({ total_active: 5, total_failing: 3, total_disabled: 1 });
  return Promise.resolve(null);
});
vi.mock('../../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string])) }));

const mockDepRows: DepRow[] = [];
vi.mock('../../hooks/use-blind-spots-data', () => ({
  useBlindSpotsData: () => ({ depRows: mockDepRows, unmatchedSignals: [], recommendations: [] }),
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function stateWith(report: unknown): Record<string, unknown> {
  return {
    blindSpotReport: report,
    blindSpotsLoading: false, blindSpotsError: null, blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(), settings: {},
  };
}

const BANNED = [
  'blindspots.scoreContext.building',
  'blindspots.score.building',
  'blindspots.empty',
  'blindspots.sourceHealth.failing',
];

function expectNoBannedText(container: HTMLElement) {
  for (const key of BANNED) expect(container.textContent ?? '').not.toContain(key);
  expect(container.textContent ?? '').not.toMatch(/\/100/);
}

beforeEach(() => {
  coldStart = false;
  cmdMock.mockClear();
});

describe('BlindSpotsView — cold start renders no banned empty state', () => {
  it('a -1 (not-enough-data) report renders no building/check-back panel and no score', () => {
    mockState = stateWith({ items: [], score: -1, total_tracked: 0, weak_match_count: 0, data_freshness: null });
    const { container } = render(<BlindSpotsView />);
    expectNoBannedText(container);
  });

  it('a cold-start profile with an empty report claims nothing', () => {
    coldStart = true;
    mockState = stateWith({ items: [], score: 0, total_tracked: 0, weak_match_count: 0, data_freshness: null });
    const { container } = render(<BlindSpotsView />);
    expectNoBannedText(container);
  });

  it('no report at all renders nothing, not "no gaps detected"', () => {
    mockState = stateWith(null);
    const { container } = render(<BlindSpotsView />);
    expect(container).toBeEmptyDOMElement();
  });

  it('never renders the gap-pressure score or the adapter-failing banner', async () => {
    mockState = stateWith({ items: [], score: 27, total_tracked: 40, weak_match_count: 0, data_freshness: null });
    const { container } = render(<BlindSpotsView />);
    await new Promise(r => setTimeout(r, 20));
    expect(container.textContent ?? '').not.toMatch(/27\s*\/100/);
    expect(cmdMock).not.toHaveBeenCalledWith('get_source_health');
    expect(screen.getByText('blindspots.stats.tracked')).toBeInTheDocument();
  });
});

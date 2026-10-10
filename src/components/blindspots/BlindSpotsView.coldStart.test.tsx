// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
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
    // With no findings the tracked count frames nothing and informs no action.
    expect(screen.queryByText('blindspots.stats.tracked')).toBeNull();
  });
});

// Fresh-profile E2E 2026-10-09: day one gave the user nothing to act on — a
// heading over a blank page with nothing scanned, then a lone "94 direct
// dependencies" bar (score -1) after a scan.
describe('BlindSpotsView — day-one states are actionable', () => {
  it('with nothing scanned, offers the project scan', () => {
    mockState = stateWith({ items: [], score: -1, total_tracked: 0, weak_match_count: 0, data_freshness: null });
    render(<BlindSpotsView />);
    expect(screen.getByTestId('blindspots-scan-prompt')).toBeInTheDocument();
    expect(screen.getByText('onboarding.choice.scanProjects')).toBeInTheDocument();
    expect(screen.queryByText('blindspots.empty')).toBeNull();
  });

  it('runs the local discovery scan and reloads the report when clicked', async () => {
    const runAutoDiscovery = vi.fn(() => Promise.resolve());
    const loadUserContext = vi.fn(() => Promise.resolve());
    const loadBlindSpots = vi.fn();
    mockState = {
      ...stateWith({ items: [], score: -1, total_tracked: 0, weak_match_count: 0, data_freshness: null }),
      runAutoDiscovery, loadUserContext, loadBlindSpots, isScanning: false,
    };
    render(<BlindSpotsView />);
    loadBlindSpots.mockClear();
    fireEvent.click(screen.getByText('onboarding.choice.scanProjects'));
    await waitFor(() => expect(loadBlindSpots).toHaveBeenCalled());
    expect(runAutoDiscovery).toHaveBeenCalled();
    expect(loadUserContext).toHaveBeenCalled();
  });

  it('offers the Settings > Projects route for choosing folders', () => {
    const setShowSettings = vi.fn();
    const setSettingsInitialTab = vi.fn();
    mockState = {
      ...stateWith({ items: [], score: -1, total_tracked: 0, weak_match_count: 0, data_freshness: null }),
      setShowSettings, setSettingsInitialTab,
    };
    render(<BlindSpotsView />);
    fireEvent.click(screen.getByText('blindspots.firstDay.chooseFolders'));
    expect(setSettingsInitialTab).toHaveBeenCalledWith('projects');
    expect(setShowSettings).toHaveBeenCalledWith(true);
  });

  it('scanned but not yet assessed (score -1): says what will appear, no bare count', () => {
    const openPreemption = vi.fn();
    mockState = {
      ...stateWith({ items: [], score: -1, total_tracked: 94, weak_match_count: 0, data_freshness: null }),
      openPreemption,
    };
    const { container } = render(<BlindSpotsView />);
    expect(screen.getByTestId('blindspots-assessing')).toBeInTheDocument();
    expect(screen.queryByText('blindspots.stats.tracked')).toBeNull();
    expect(container.textContent ?? '').not.toContain('94');
    expect(screen.queryByTestId('blindspots-scan-prompt')).toBeNull();
    // AD-054: Blind Spots sits inside Preemption — the link selects the worklist sub-view.
    fireEvent.click(screen.getByText('blindspots.firstDay.openWorklist'));
    expect(openPreemption).toHaveBeenCalledWith('worklist');
  });
});

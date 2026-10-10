// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PreemptionWorklist } from './PreemptionWorklist';

// Fresh-profile E2E 2026-10-10: a profile with NO dependencies was told "Your
// stack is clean — Intelligence is actively monitoring". An empty worklist is
// "no threats" only when there is a stack; with none known it asks for the
// project scan (the Blind Spots day-one flow, #888).

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

const loadPreemption = vi.fn();
const runAutoDiscovery = vi.fn(() => Promise.resolve());
const loadUserContext = vi.fn(() => Promise.resolve());

function setFeed(totalTracked: number | null) {
  mockState = {
    preemptionFeed: {
      items: [],
      total: 0,
      critical_count: 0,
      high_count: 0,
      score: null,
      total_tracked: totalTracked,
      weak_match_count: null,
      data_freshness: null,
      tier_scope: 'full',
    },
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    preemptionLastDismissed: null,
    loadPreemption,
    refreshPreemptionQuietly: vi.fn(),
    dismissPreemptionItem: vi.fn(),
    undoPreemptionDismissal: vi.fn(),
    clearPreemptionUndo: vi.fn(),
    expandPreemptionPlan: vi.fn(),
    runAutoDiscovery,
    loadUserContext,
    isScanning: false,
    setShowSettings: vi.fn(),
    setSettingsInitialTab: vi.fn(),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('PreemptionWorklist — empty state', () => {
  it('asks for a project scan when no dependencies are known', async () => {
    setFeed(0);
    render(<PreemptionWorklist />);

    const prompt = screen.getByTestId('preemption-scan-prompt');
    expect(prompt).toHaveTextContent('preemption.noDeps.title');
    expect(screen.queryByText('preemption.empty.title')).toBeNull();

    loadPreemption.mockClear();
    fireEvent.click(screen.getByText('onboarding.choice.scanProjects'));
    await waitFor(() => expect(loadPreemption).toHaveBeenCalled());
    expect(runAutoDiscovery).toHaveBeenCalled();
  });

  it('says the stack is clean only when dependencies are known', () => {
    setFeed(166);
    render(<PreemptionWorklist />);
    expect(screen.getByText('preemption.empty.title')).toBeInTheDocument();
    expect(screen.queryByTestId('preemption-scan-prompt')).toBeNull();
  });

  it('keeps the no-threats copy when the dependency count could not be read', () => {
    setFeed(null);
    render(<PreemptionWorklist />);
    expect(screen.getByText('preemption.empty.title')).toBeInTheDocument();
    expect(screen.queryByTestId('preemption-scan-prompt')).toBeNull();
  });
});

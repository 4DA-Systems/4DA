// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';

// AB-011 render guard (Blind Spots sibling of the Preemption paywall fix):
// blind-spots-slice.test.ts proves the gate → blindSpotsPaywalled wiring; this
// proves the view renders the upgrade CTA for that flag instead of the red
// "Something went wrong" error banner a free-tier user used to get.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../../hooks/use-blind-spots-data', () => ({
  useBlindSpotsData: () => ({ depRows: [], unmatchedSignals: [], recommendations: [] }),
}));
vi.mock('../../lib/commands', () => ({
  cmd: vi.fn().mockResolvedValue({ total_active: 0, total_failing: 0, total_disabled: 0 }),
}));
vi.mock('../../lib/trust-feedback', () => ({ recordTrustEvent: vi.fn() }));
vi.mock('./dismissal-utils', () => ({
  loadPersistedDismissals: () => new Set<string>(),
  persistDismissal: vi.fn(),
  removeDismissal: vi.fn(),
}));
vi.mock('../SignalUpgradeCTA', () => ({
  SignalUpgradeCTA: () => <div data-testid="signal-upgrade-cta" />,
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function setState(overrides: Record<string, unknown>) {
  mockState = {
    blindSpotReport: null,
    blindSpotsLoading: false,
    blindSpotsError: null,
    blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(),
    ...overrides,
  };
}

describe('BlindSpotsView — paywall render', () => {
  it('renders the upgrade CTA + lock copy when paywalled, NOT an error banner', () => {
    setState({ blindSpotsPaywalled: true });
    render(<BlindSpotsView />);

    expect(screen.getByText('blindspots.locked.title')).toBeInTheDocument();
    expect(screen.getByText('blindspots.locked.subtitle')).toBeInTheDocument();
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
    // the generic error fallback must NOT appear
    expect(screen.queryByText('blindspots.error.title')).toBeNull();
  });

  it('renders the error path (not the CTA) for a genuine fault', () => {
    setState({ blindSpotsError: 'database is locked' });
    render(<BlindSpotsView />);
    expect(screen.getByText('blindspots.error.title')).toBeInTheDocument();
    expect(screen.queryByTestId('signal-upgrade-cta')).toBeNull();
  });
});

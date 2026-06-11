// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';

// AB-011 render guard (Blind Spots sibling of the Preemption paywall fix):
// blind-spots-slice.test.ts proves the gate → blindSpotsPaywalled wiring; this
// proves the view renders the upgrade CTA for that flag instead of the red
// "Something went wrong" error banner a free-tier user used to get.
//
// Tier rebalance (2026-06-12): the paywalled state now carries an honest free
// teaser — real counts from get_blind_spot_teaser rendered above the CTA when
// nonzero. Zero counts or cold_start render the plain paywall unchanged
// (doctrine rule 6: no "no data yet" states).

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../../hooks/use-blind-spots-data', () => ({
  useBlindSpotsData: () => ({ depRows: [], unmatchedSignals: [], recommendations: [] }),
}));

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
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

interface TeaserShape {
  uncovered_count: number;
  stale_topic_count: number;
  missed_signal_count: number;
  cold_start: boolean;
}

function mockCommands(teaser: TeaserShape | 'reject') {
  cmdMock.mockImplementation((name: string) => {
    if (name === 'get_blind_spot_teaser') {
      return teaser === 'reject'
        ? Promise.reject(new Error('backend unavailable'))
        : Promise.resolve(teaser);
    }
    // get_source_health and anything else: benign empty shape.
    return Promise.resolve({ total_active: 0, total_failing: 0, total_disabled: 0 });
  });
}

beforeEach(() => {
  cmdMock.mockReset();
});

describe('BlindSpotsView — paywall render', () => {
  it('renders the upgrade CTA + lock copy when paywalled, NOT an error banner', async () => {
    mockCommands({ uncovered_count: 0, stale_topic_count: 0, missed_signal_count: 0, cold_start: false });
    setState({ blindSpotsPaywalled: true });
    render(<BlindSpotsView />);

    expect(screen.getByText('blindspots.locked.title')).toBeInTheDocument();
    expect(screen.getByText('blindspots.locked.subtitle')).toBeInTheDocument();
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
    // the generic error fallback must NOT appear
    expect(screen.queryByText('blindspots.error.title')).toBeNull();
  });

  it('renders the error path (not the CTA) for a genuine fault', () => {
    mockCommands('reject');
    setState({ blindSpotsError: 'database is locked' });
    render(<BlindSpotsView />);
    expect(screen.getByText('blindspots.error.title')).toBeInTheDocument();
    expect(screen.queryByTestId('signal-upgrade-cta')).toBeNull();
  });
});

describe('BlindSpotsView — free teaser on the paywall', () => {
  it('shows real counts above the CTA when the teaser is nonzero', async () => {
    mockCommands({ uncovered_count: 7, stale_topic_count: 2, missed_signal_count: 5, cold_start: false });
    setState({ blindSpotsPaywalled: true });
    render(<BlindSpotsView />);

    expect(await screen.findByTestId('blindspots-teaser')).toBeInTheDocument();
    // i18n test setup returns keys; the counts ride the interpolation params,
    // so presence of all three line keys proves each nonzero count rendered.
    expect(screen.getByText('blindspots.teaser.uncovered')).toBeInTheDocument();
    expect(screen.getByText('blindspots.teaser.staleTopics')).toBeInTheDocument();
    expect(screen.getByText('blindspots.teaser.missedSignals')).toBeInTheDocument();
    // The lock copy + CTA still render — teaser augments, never replaces.
    expect(screen.getByText('blindspots.locked.title')).toBeInTheDocument();
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
  });

  it('renders only the nonzero count lines', async () => {
    mockCommands({ uncovered_count: 3, stale_topic_count: 0, missed_signal_count: 0, cold_start: false });
    setState({ blindSpotsPaywalled: true });
    render(<BlindSpotsView />);

    expect(await screen.findByTestId('blindspots-teaser')).toBeInTheDocument();
    expect(screen.getByText('blindspots.teaser.uncovered')).toBeInTheDocument();
    expect(screen.queryByText('blindspots.teaser.staleTopics')).toBeNull();
    expect(screen.queryByText('blindspots.teaser.missedSignals')).toBeNull();
  });

  it('renders the plain paywall unchanged when cold_start (doctrine rule 6)', async () => {
    mockCommands({ uncovered_count: 0, stale_topic_count: 0, missed_signal_count: 0, cold_start: true });
    setState({ blindSpotsPaywalled: true });
    render(<BlindSpotsView />);

    expect(screen.getByText('blindspots.locked.title')).toBeInTheDocument();
    // Let the teaser fetch settle, then confirm nothing extra rendered.
    await Promise.resolve();
    expect(screen.queryByTestId('blindspots-teaser')).toBeNull();
  });

  it('renders the plain paywall when the teaser call fails', async () => {
    mockCommands('reject');
    setState({ blindSpotsPaywalled: true });
    render(<BlindSpotsView />);

    expect(screen.getByText('blindspots.locked.title')).toBeInTheDocument();
    await Promise.resolve();
    expect(screen.queryByTestId('blindspots-teaser')).toBeNull();
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
  });
});

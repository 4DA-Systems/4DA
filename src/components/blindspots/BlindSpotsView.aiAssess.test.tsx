// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';
import type { DepRow } from './types';

// Phase B: the "Assess with AI" button runs a batched triage; the surfaced gap
// deps then regroup into "Worth reviewing" vs a collapsed "Probably fine" bucket,
// each row annotated with the model's one-line recommendation.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../../lib/trust-feedback', () => ({ recordTrustEvent: vi.fn() }));
vi.mock('./dismissal-utils', () => ({
  loadPersistedDismissals: () => new Set<string>(),
  persistDismissal: vi.fn(),
  removeDismissal: vi.fn(),
}));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./ScoreBar', () => ({ default: () => <div /> }));

// Stub the section renderers to expose which deps each received.
vi.mock('./StackCoverageMap', () => ({
  TierSection: ({ title, depRows }: { title: string; depRows: DepRow[] }) => (
    <div data-testid="tier" data-title={title}>{depRows.map(d => <span key={d.name} data-testid="worth-dep">{d.name}</span>)}</div>
  ),
  EmergingSignals: () => null,
}));
vi.mock('./CollapsedSections', () => ({
  CoveredSection: () => null,
  OtherBuildTargetsSection: () => null,
  ProbablyFineSection: ({ depRows }: { depRows: DepRow[] }) => (
    <div data-testid="probably-fine">{depRows.map(d => <span key={d.name} data-testid="fine-dep">{d.name}</span>)}</div>
  ),
}));

const assessment = {
  assessments: [
    { dep_name: 'react (npm)', worth_reviewing: true, recommendation: 'Review v19 breaking changes.' },
    { dep_name: 'ammonia (crates.io)', worth_reviewing: false, recommendation: 'Stable sanitizer, no action.' },
  ],
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

beforeEach(() => {
  cmdMock.mockClear();
  mockDepRows = [depRow('react (npm)'), depRow('ammonia (crates.io)')];
  mockState = {
    blindSpotReport: { items: [], score: 50, total_tracked: 2, weak_match_count: 0, data_freshness: null },
    blindSpotsLoading: false, blindSpotsError: null, blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(),
  };
});

describe('BlindSpotsView — AI triage (Phase B)', () => {
  it('shows the Assess-with-AI button when there are blind spots', () => {
    render(<BlindSpotsView />);
    expect(screen.getByText('blindspots.ai.assess')).toBeInTheDocument();
  });

  it('regroups into worth-reviewing vs probably-fine after assessing', async () => {
    render(<BlindSpotsView />);
    fireEvent.click(screen.getByText('blindspots.ai.assess'));

    await waitFor(() => expect(screen.getByTestId('probably-fine')).toBeInTheDocument());

    const worth = screen.queryAllByTestId('worth-dep').map(n => n.textContent);
    const fine = screen.queryAllByTestId('fine-dep').map(n => n.textContent);
    expect(worth).toContain('react (npm)');
    expect(worth).not.toContain('ammonia (crates.io)');
    expect(fine).toContain('ammonia (crates.io)');
    expect(fine).not.toContain('react (npm)');
    expect(cmdMock).toHaveBeenCalledWith('assess_blind_spots_with_ai');
  });

  it('keeps an unassessed dep visible (defaults to worth-reviewing, never hidden)', async () => {
    mockDepRows = [depRow('react (npm)'), depRow('ammonia (crates.io)'), depRow('unjudged (npm)')];
    render(<BlindSpotsView />);
    fireEvent.click(screen.getByText('blindspots.ai.assess'));
    await waitFor(() => expect(screen.getByTestId('probably-fine')).toBeInTheDocument());
    const worth = screen.queryAllByTestId('worth-dep').map(n => n.textContent);
    expect(worth).toContain('unjudged (npm)'); // not in the assessment -> stays surfaced
  });
});

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';
import type { DepRow } from './types';

// 2026-08-31 live audit: the "Unreviewed Signals … unreviewed activity —
// Drifting" tier was inflated with rows whose own explanations read "Sources
// were checked but found no results". Zero-coverage rows now render in their
// own NoCoverageSection with their own count; the drifting tier and its
// counts carry only deps with REAL unreviewed activity.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn(() => Promise.resolve({ total_active: 0, total_failing: 0, total_disabled: 0 })) }));
vi.mock('../../lib/trust-feedback', () => ({ recordTrustEvent: vi.fn() }));
vi.mock('./dismissal-utils', () => ({
  loadPersistedDismissals: () => new Set<string>(),
  persistDismissal: vi.fn(),
  removeDismissal: vi.fn(),
}));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./ScoreBar', () => ({ default: () => <div /> }));

// Stub the section renderers so we can assert which deps each section received.
vi.mock('./StackCoverageMap', () => ({
  TierSection: ({ depRows, subtitle }: { depRows: DepRow[]; subtitle: string }) => (
    <div data-testid="tier-section" data-subtitle={subtitle}>
      {depRows.map(d => <span key={d.name} data-testid="tier-dep">{d.name}</span>)}
    </div>
  ),
  EmergingSignals: () => null,
}));
vi.mock('./CollapsedSections', () => ({
  CoveredSection: () => null,
  NoCoverageSection: ({ depRows }: { depRows: DepRow[] }) => (
    <div data-testid="nocov-section">{depRows.map(d => <span key={d.name} data-testid="nocov-dep">{d.name}</span>)}</div>
  ),
  OtherBuildTargetsSection: () => null,
  ProbablyFineSection: () => null,
}));

let mockDepRows: DepRow[] = [];
vi.mock('../../hooks/use-blind-spots-data', () => ({
  useBlindSpotsData: () => ({ depRows: mockDepRows, unmatchedSignals: [], recommendations: [] }),
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function gap(id: string, noCoverage: boolean) {
  return {
    id: `bs_uncov_npm_${id}`, kind: 'gap', title: id, explanation: '',
    confidence: { value: 0.4, provenance: 'heuristic', sample_size: null },
    urgency: 'medium', reversibility: null, evidence: [],
    affected_projects: [], affected_deps: [id], suggested_actions: [],
    precedents: [], refutation_condition: null,
    lens_hints: { briefing: false, preemption: false, blind_spots: true, evidence: false, other_build_target: false, upgrade_plan: false, no_coverage: noCoverage },
    created_at: 0, expires_at: null,
  } as unknown as DepRow['gap'];
}

function depRow(name: string, status: DepRow['status']): DepRow {
  return { name, status, urgency: 'medium', gap: gap(name, status === 'no_coverage'), signals: [], projects: [] };
}

beforeEach(() => {
  mockState = {
    blindSpotReport: { items: [], score: 50, total_tracked: 4, weak_match_count: 0, data_freshness: null },
    blindSpotsLoading: false,
    blindSpotsError: null,
    blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(),
  };
});

describe('BlindSpotsView — no-coverage section split (2026-08-31 audit)', () => {
  it('routes zero-coverage deps to NoCoverageSection, out of the drifting tier', () => {
    mockDepRows = [
      depRow('react (npm)', 'falling_behind'),
      depRow('quiet-a (npm)', 'no_coverage'),
      depRow('quiet-b (npm)', 'no_coverage'),
    ];
    render(<BlindSpotsView />);

    const tierDeps = screen.queryAllByTestId('tier-dep').map(n => n.textContent);
    const nocovDeps = screen.queryAllByTestId('nocov-dep').map(n => n.textContent);
    expect(tierDeps).toEqual(['react (npm)']);
    expect(nocovDeps).toEqual(['quiet-a (npm)', 'quiet-b (npm)']);
  });

  it('drifting tier count reflects only real unreviewed activity', () => {
    mockDepRows = [
      depRow('react (npm)', 'falling_behind'),
      depRow('vue (npm)', 'falling_behind'),
      depRow('quiet-a (npm)', 'no_coverage'),
    ];
    render(<BlindSpotsView />);

    // The ecosystem/drifting TierSection's subtitle is built from
    // ecosystemDeps.length — 2 here, never 3 (the mocked t() returns the key;
    // the count rides in the interpolation options, so assert via the rows).
    const tierSections = screen.getAllByTestId('tier-section');
    expect(tierSections).toHaveLength(1);
    expect(tierSections[0]!.querySelectorAll('[data-testid="tier-dep"]')).toHaveLength(2);
    // Summary bar renders a distinct no-coverage chip with its own count.
    expect(screen.getByText(/1 blindspots\.status\.nocoverage/i)).toBeInTheDocument();
  });

  it('renders no NoCoverageSection deps when every gap has activity', () => {
    mockDepRows = [depRow('react (npm)', 'falling_behind')];
    render(<BlindSpotsView />);
    expect(screen.queryAllByTestId('nocov-dep')).toHaveLength(0);
  });
});

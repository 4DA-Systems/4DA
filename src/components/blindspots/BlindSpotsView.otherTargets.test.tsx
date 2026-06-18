// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';
import type { DepRow } from './types';

// Phase 2c: dependency coverage gaps that apply only to a build target the user
// does not build on the host are pulled OUT of the stack/ecosystem tiers into a
// collapsed "Other build targets" group — surfaced, de-prioritised, never hidden.

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
  TierSection: ({ depRows }: { depRows: DepRow[] }) => (
    <div data-testid="tier-section">{depRows.map(d => <span key={d.name} data-testid="tier-dep">{d.name}</span>)}</div>
  ),
  EmergingSignals: () => null,
}));
vi.mock('./CollapsedSections', () => ({
  CoveredSection: ({ depRows }: { depRows: DepRow[] }) => (
    <div>{depRows.map(d => <span key={d.name} data-testid="covered-dep">{d.name}</span>)}</div>
  ),
  OtherBuildTargetsSection: ({ depRows }: { depRows: DepRow[] }) => (
    <div data-testid="other-section">{depRows.map(d => <span key={d.name} data-testid="other-dep">{d.name}</span>)}</div>
  ),
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

function gap(id: string, otherBuildTarget: boolean) {
  return {
    id, kind: 'gap', title: id, explanation: '',
    confidence: { value: 0.4, provenance: 'heuristic', sample_size: null },
    urgency: 'high', reversibility: null, evidence: [],
    affected_projects: [], affected_deps: [id], suggested_actions: [],
    precedents: [], refutation_condition: null,
    lens_hints: { briefing: false, preemption: false, blind_spots: true, evidence: false, other_build_target: otherBuildTarget },
    created_at: 0, expires_at: null,
  } as unknown as DepRow['gap'];
}

function depRow(name: string, status: DepRow['status'], otherBuildTarget: boolean): DepRow {
  return { name, status, urgency: 'high', gap: gap(name, otherBuildTarget), signals: [], projects: [] };
}

beforeEach(() => {
  mockState = {
    blindSpotReport: { items: [], score: 50, total_tracked: 2, weak_match_count: 0, data_freshness: null },
    blindSpotsLoading: false,
    blindSpotsError: null,
    blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(),
  };
});

describe('BlindSpotsView — other-build-targets partition (Phase 2c)', () => {
  it('routes platform-inactive deps to the Other-build-targets section, not the stack tier', () => {
    mockDepRows = [depRow('react', 'blind_spot', false), depRow('libc', 'blind_spot', true)];
    render(<BlindSpotsView />);

    const tierDeps = screen.queryAllByTestId('tier-dep').map(n => n.textContent);
    const otherDeps = screen.queryAllByTestId('other-dep').map(n => n.textContent);
    expect(tierDeps).toContain('react');
    expect(tierDeps).not.toContain('libc');
    expect(otherDeps).toContain('libc');
    expect(otherDeps).not.toContain('react');
  });

  it('renders no other-section deps when nothing is platform-inactive', () => {
    mockDepRows = [depRow('react', 'blind_spot', false)];
    render(<BlindSpotsView />);
    expect(screen.queryAllByTestId('other-dep')).toHaveLength(0);
  });
});

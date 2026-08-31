// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import BlindSpotsView from './BlindSpotsView';
import type { DepRow } from './types';

// 2026-08-31 live audit (Victauri `audit_accessibility`, Blind Spots tab):
// `heading-order`, moderate — "Heading level skipped from h1 to h3". The panel
// title rendered as a SECOND <h1> ("Coverage Gaps") next to App.tsx's sr-only
// document <h1> ("4DA"), while every section header underneath was an <h3>.
//
// The panel title is now an <h2>, so the tree steps h1 -> h2 -> h3 with no
// skipped level, and the view no longer competes for the document h1.
//
// These tests render the REAL TierSection / EmergingSignals / CollapsedSections
// components (not stubs) — the point is the heading tags they actually emit.

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
// DepCoverageRow pulls translated content; it renders no headings of its own.
vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({
    getTranslated: (_id: string, fallback: string) => fallback,
    requestTranslation: vi.fn(),
  }),
}));

let mockDepRows: DepRow[] = [];
vi.mock('../../hooks/use-blind-spots-data', () => ({
  useBlindSpotsData: () => ({ depRows: mockDepRows, unmatchedSignals: [], recommendations: [] }),
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function gap(id: string, otherBuildTarget: boolean, noCoverage: boolean) {
  return {
    id: `bs_uncov_npm_${id}`, kind: 'gap', title: id, explanation: '',
    confidence: { value: 0.4, provenance: 'heuristic', sample_size: null },
    urgency: 'medium', reversibility: null, evidence: [],
    affected_projects: [], affected_deps: [id], suggested_actions: [],
    precedents: [], refutation_condition: null,
    lens_hints: {
      briefing: false, preemption: false, blind_spots: true, evidence: false,
      other_build_target: otherBuildTarget, upgrade_plan: false, no_coverage: noCoverage,
    },
    created_at: 0, expires_at: null,
  } as unknown as DepRow['gap'];
}

function depRow(name: string, status: DepRow['status'], otherBuildTarget = false): DepRow {
  return {
    name, status, urgency: 'medium',
    gap: gap(name, otherBuildTarget, status === 'no_coverage'),
    signals: [], projects: [],
  };
}

/** Every heading the view renders, in document order, as numeric levels. */
function headingLevels(container: HTMLElement): number[] {
  return Array.from(container.querySelectorAll('h1,h2,h3,h4,h5,h6'))
    .map(el => Number(el.tagName.slice(1)));
}

/**
 * Mirrors axe-core's `heading-order` rule. App.tsx renders the document's only
 * <h1> ("4DA", sr-only) outside this subtree, so the walk starts at level 1 and
 * each heading may descend by at most one level from the previous one.
 */
function expectNoSkippedHeadingLevel(container: HTMLElement) {
  let previous = 1; // App.tsx's sr-only document <h1>
  for (const level of headingLevels(container)) {
    expect(level).toBeLessThanOrEqual(previous + 1);
    previous = level;
  }
}

beforeEach(() => {
  mockDepRows = [];
  mockState = {
    blindSpotReport: { items: [], score: 50, total_tracked: 5, weak_match_count: 0, data_freshness: null },
    blindSpotsLoading: false,
    blindSpotsError: null,
    blindSpotsPaywalled: false,
    loadBlindSpots: vi.fn(),
  };
});

describe('BlindSpotsView — heading hierarchy (2026-08-31 a11y audit)', () => {
  it('titles the panel with an h2, never a second document h1', () => {
    mockDepRows = [depRow('react (npm)', 'blind_spot')];
    const { container } = render(<BlindSpotsView />);

    expect(container.querySelectorAll('h1')).toHaveLength(0);
    const title = container.querySelector('h2');
    expect(title).not.toBeNull();
    expect(title!.textContent).toBe('blindspots.title');
  });

  it('renders every section header one level below the panel title', () => {
    mockDepRows = [
      depRow('react (npm)', 'blind_spot'),
      depRow('vue (npm)', 'falling_behind'),
      depRow('quiet (npm)', 'no_coverage'),
      depRow('lodash (npm)', 'well_covered'),
      depRow('libc (crates.io)', 'blind_spot', true),
    ];
    const { container } = render(<BlindSpotsView />);

    // The panel title is the only h2; all five sections are h3 siblings.
    expect(container.querySelectorAll('h2')).toHaveLength(1);
    const sectionHeadings = Array.from(container.querySelectorAll('h3'));
    expect(sectionHeadings.length).toBeGreaterThanOrEqual(4);
    expectNoSkippedHeadingLevel(container);
  });

  it('keeps the hierarchy intact in the error state', () => {
    mockState = { ...mockState, blindSpotsError: 'boom' };
    const { container } = render(<BlindSpotsView />);

    expect(container.querySelectorAll('h1')).toHaveLength(0);
    expect(container.querySelector('h2')!.textContent).toBe('blindspots.title');
    expectNoSkippedHeadingLevel(container);
  });

  it('keeps the hierarchy intact in the empty state', () => {
    mockState = {
      ...mockState,
      blindSpotReport: { items: [], score: 5, total_tracked: 0, weak_match_count: 0, data_freshness: null },
    };
    const { container } = render(<BlindSpotsView />);

    expect(container.querySelectorAll('h1')).toHaveLength(0);
    expectNoSkippedHeadingLevel(container);
  });

  it('keeps the hierarchy intact behind the paywall', () => {
    mockState = { ...mockState, blindSpotsPaywalled: true };
    const { container } = render(<BlindSpotsView />);

    expect(container.querySelectorAll('h1')).toHaveLength(0);
    expect(container.querySelector('h2')!.textContent).toBe('blindspots.title');
    expectNoSkippedHeadingLevel(container);
  });
});

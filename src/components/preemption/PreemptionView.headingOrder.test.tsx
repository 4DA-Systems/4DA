// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// 2026-08-31 live audit (Victauri `audit_accessibility`) found `heading-order`,
// moderate — "Heading level skipped from h1 to h3" — on Blind Spots. Preemption
// carried the identical defect: the panel title rendered as a SECOND <h1>
// ("Preemption") beside App.tsx:352's sr-only document <h1> ("4DA"), while every
// tier section under it was an <h3>, leaving no h2 in the subtree.
//
// The panel title is now an <h2>, so the tree steps h1 -> h2 -> h3 with no
// skipped level. PreemptionView has a SINGLE return whose <header> renders in
// every state (paywall, loading, error, empty, main, free-floor), so one change
// covers them all — these tests pin each state.
//
// These render the REAL PreemptionTierSection / ItemCard / EvidenceList
// components (not stubs), because the point is the heading tags they emit.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn(() => Promise.resolve(null)) }));
vi.mock('../../lib/trust-feedback', () => ({ recordTrustEvent: vi.fn() }));
// ItemCard pulls translated content; it renders no headings of its own beyond
// the card title asserted below.
vi.mock('../ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({
    getTranslated: (_id: string, fallback: string) => fallback,
    requestTranslation: vi.fn(),
  }),
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function setState(overrides: Record<string, unknown>) {
  mockState = {
    preemptionFeed: null,
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    preemptionLastDismissed: null,
    loadPreemption: vi.fn(),
    dismissPreemptionItem: vi.fn(),
    undoPreemptionDismissal: vi.fn(),
    clearPreemptionUndo: vi.fn(),
    expandPreemptionPlan: vi.fn(),
    ...overrides,
  };
}

function makeItem(
  id: string,
  provenance: string,
  hints: { upgrade_plan?: boolean; other_build_target?: boolean } = {},
) {
  return {
    id, kind: 'alert', title: `CVE in ${id}`,
    explanation: 'version-range match',
    confidence: { value: 0.9, provenance, sample_size: null },
    urgency: 'high', reversibility: null,
    // A citation whose title differs from the card title renders EvidenceList's h4.
    evidence: [{ source: 'osv', title: `${id} advisory`, url: 'https://osv.dev', freshness_days: 1, relevance_note: null }],
    affected_projects: [], affected_deps: [id], suggested_actions: [],
    precedents: [], refutation_condition: null,
    lens_hints: {
      briefing: false, preemption: true, blind_spots: false, evidence: false,
      other_build_target: hints.other_build_target ?? false,
      upgrade_plan: hints.upgrade_plan ?? false,
      no_coverage: false,
    },
    created_at: 0, expires_at: null,
  };
}

function makeFeed(items: ReturnType<typeof makeItem>[]) {
  return {
    items, total: items.length,
    critical_count: 0, high_count: items.length,
    score: null, total_tracked: null, weak_match_count: null,
    data_freshness: null, tier_scope: 'full',
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

describe('PreemptionView — heading hierarchy (2026-08-31 a11y audit)', () => {
  it('titles the panel with an h2, never a second document h1', () => {
    setState({ preemptionFeed: makeFeed([makeItem('axios', 'osv_verified')]) });
    const { container } = render(<PreemptionView />);

    expect(container.querySelectorAll('h1')).toHaveLength(0);
    const title = container.querySelector('h2');
    expect(title).not.toBeNull();
    expect(title!.textContent).toBe('preemption.title');
  });

  it('renders every tier section one level below the panel title', () => {
    setState({
      preemptionFeed: makeFeed([
        makeItem('plan-step', 'llm_assessed', { upgrade_plan: true }),
        makeItem('axios', 'osv_verified'),
        makeItem('lodash', 'llm_assessed'),
        makeItem('leftpad', 'heuristic'),
        makeItem('libc', 'osv_verified', { other_build_target: true }),
      ]),
    });
    const { container } = render(<PreemptionView />);

    // One h2 (the panel title); the five tier/group headers are all h3.
    expect(container.querySelectorAll('h2')).toHaveLength(1);
    expect(container.querySelectorAll('h3').length).toBeGreaterThanOrEqual(5);
    expectNoSkippedHeadingLevel(container);
  });

  it('keeps the hierarchy intact with card and evidence headings nested in a tier', () => {
    setState({ preemptionFeed: makeFeed([makeItem('axios', 'osv_verified')]) });
    const { container } = render(<PreemptionView />);

    // Tier header (h3) -> card title (h3) -> evidence header (h4): descends by
    // at most one at each step, so axe's heading-order rule is satisfied.
    expect(container.querySelectorAll('h4').length).toBeGreaterThanOrEqual(1);
    expectNoSkippedHeadingLevel(container);
  });

  it.each([
    ['paywall', { preemptionPaywalled: true }],
    ['loading', { preemptionLoading: true }],
    ['error', { preemptionError: 'database is locked' }],
    ['empty', { preemptionFeed: makeFeed([]) }],
    ['free floor', { preemptionFeed: { ...makeFeed([makeItem('axios', 'osv_verified')]), tier_scope: 'free_floor' } }],
  ])('keeps the hierarchy intact in the %s state', (_label, overrides) => {
    setState(overrides);
    const { container } = render(<PreemptionView />);

    expect(container.querySelectorAll('h1')).toHaveLength(0);
    expect(container.querySelector('h2')!.textContent).toBe('preemption.title');
    expectNoSkippedHeadingLevel(container);
  });
});

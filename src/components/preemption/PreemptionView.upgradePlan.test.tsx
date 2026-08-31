// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// Phase 1 dependency intelligence: items with lens_hints.upgrade_plan render in
// a dedicated "Upgrade Plan" section above the tiers.
//
// AD-035 (2026-08-31): the view GROUPS and nothing else. The visibility filter
// (local dismissals, plan-covered per-package alerts regrouped away) and the
// summary counts live in ONE backend definition
// (src-tauri/src/evidence/list_transport.rs, pinned by its own tests) — the
// view renders every item the feed contains and echoes the feed's counts
// verbatim. These tests pin that the view does NOT re-filter or re-count
// (client-side copies of the filter are exactly what produced the audit's
// 12/41/120-vs-15/67/149 header drift).

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./PreemptionFreeFloorNotice', () => ({ PreemptionFreeFloorNotice: () => <div /> }));

vi.mock('./PreemptionTierSection', () => ({
  PreemptionTierSection: ({ title, items, subtitle, hiddenExtra }: { title: string; items: Array<{ id: string }>; subtitle?: string; hiddenExtra?: number }) => (
    <div data-testid="tier-section" data-title={title} data-subtitle={subtitle} data-hidden-extra={hiddenExtra ?? 0}>
      {items.map((i) => <span key={i.id} data-testid="tier-item" data-section={title}>{i.id}</span>)}
    </div>
  ),
}));

vi.mock('./PreemptionCard', () => ({
  URGENCY_ORDER: ['critical', 'high', 'medium', 'watch'],
  ItemCard: ({ item }: { item: { id: string } }) => <div data-testid="other-card">{item.id}</div>,
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

const PLAN_TITLE = 'preemption.upgradePlan.title';
const VERIFIED_TITLE = 'preemption.tier.verified';

function baseItem(id: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    kind: 'alert',
    title: `item ${id}`,
    explanation: '',
    confidence: { value: 0.9, provenance: 'osv_verified', sample_size: null },
    urgency: 'high',
    reversibility: null,
    evidence: [],
    affected_projects: [],
    affected_deps: [id],
    suggested_actions: [],
    precedents: [],
    refutation_condition: null,
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: false, no_coverage: false },
    created_at: 0,
    expires_at: null,
    ...overrides,
  };
}

/** A ranked Upgrade Plan step for `pkg` (Heuristic provenance, plan-hinted). */
function planStep(pkg: string, urgency = 'high') {
  return baseItem(`upgrade-plan:npm:${pkg}`, {
    confidence: { value: 0.9, provenance: 'heuristic', sample_size: null },
    urgency,
    affected_deps: [pkg],
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: true, no_coverage: false },
  });
}

/** The per-package OSV-verified alert for `pkg` (what the plan step regroups). */
function osvAlert(pkg: string, overrides: Record<string, unknown> = {}) {
  return baseItem(`osv-${pkg}`, { affected_deps: [pkg], ...overrides });
}

function setFeed(
  items: Array<ReturnType<typeof baseItem>>,
  tierScope: 'full' | 'free_floor' = 'full',
  feedOverrides: Record<string, unknown> = {},
) {
  mockState = {
    preemptionFeed: {
      items,
      total: items.length,
      critical_count: 0,
      high_count: 0,
      score: null,
      total_tracked: null,
      weak_match_count: null,
      data_freshness: null,
      tier_scope: tierScope,
      ...feedOverrides,
    },
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    preemptionLastDismissed: null,
    loadPreemption: vi.fn(),
    dismissPreemptionItem: vi.fn(),
    undoPreemptionDismissal: vi.fn(),
    clearPreemptionUndo: vi.fn(),
    expandPreemptionPlan: vi.fn(),
  };
}

function itemsInSection(title: string): string[] {
  return screen
    .getAllByTestId('tier-item')
    .filter((n) => n.getAttribute('data-section') === title)
    .map((n) => n.textContent ?? '');
}

describe('PreemptionView — Upgrade Plan group (Phase 1 dependency intelligence)', () => {
  it('renders plan steps in the Upgrade Plan section, not in the tiers', () => {
    setFeed([planStep('lodash'), osvAlert('axios')]);
    render(<PreemptionView />);

    expect(itemsInSection(PLAN_TITLE)).toEqual(['upgrade-plan:npm:lodash']);
    expect(itemsInSection(VERIFIED_TITLE)).toEqual(['osv-axios']);
  });

  it('renders EVERY feed item — the regroup filter lives in the backend now (AD-035)', () => {
    // Pre-AD-035 the view dropped 'osv-lodash' here as a plan-covered
    // duplicate, while the backend still counted it — the exact source of the
    // audit's header-vs-payload count drift. The one filter definition now
    // runs in get_preemption_alerts (pinned by list_transport_tests.rs);
    // whatever survives it is rendered, without exception.
    setFeed([planStep('lodash'), osvAlert('lodash'), osvAlert('axios')]);
    render(<PreemptionView />);

    expect(itemsInSection(PLAN_TITLE)).toEqual(['upgrade-plan:npm:lodash']);
    const verified = itemsInSection(VERIFIED_TITLE);
    expect(verified).toContain('osv-axios');
    expect(verified).toContain('osv-lodash');
  });

  it('echoes the feed counts verbatim — the view never re-counts', () => {
    // Deliberately inconsistent with the items array: the bar must show the
    // BACKEND numbers (same filter that chose the items), not a local tally.
    setFeed([osvAlert('axios', { urgency: 'high' })], 'full', {
      critical_count: 3,
      high_count: 7,
      total: 11,
    });
    const { container } = render(<PreemptionView />);

    expect(container.textContent).toContain('3 preemption.urgency.critical');
    expect(container.textContent).toContain('7 preemption.urgency.high');
  });

  it('derives the held-back plan tail from total - items.length and wires it to the plan section', () => {
    // The list transport ships the top plan steps and holds the collapsed
    // tail server-side; the counts still include it.
    setFeed([planStep('lodash'), osvAlert('axios')], 'full', { total: 96 });
    render(<PreemptionView />);

    const plan = screen
      .getAllByTestId('tier-section')
      .find((s) => s.getAttribute('data-title') === PLAN_TITLE);
    expect(plan?.getAttribute('data-hidden-extra')).toBe('94');
  });

  it('keeps platform-inactive alerts in the other-targets group even when covered', () => {
    setFeed([
      planStep('winapi'),
      osvAlert('winapi', {
        lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: true, upgrade_plan: false, no_coverage: false },
      }),
    ]);
    render(<PreemptionView />);

    // The other-targets group header renders (its item is grouped, not swallowed).
    expect(screen.getByText('preemption.otherTargets.show')).toBeInTheDocument();
  });

  it('renders no Upgrade Plan section when the feed has no plan steps', () => {
    setFeed([osvAlert('axios')]);
    render(<PreemptionView />);
    expect(screen.queryByText(PLAN_TITLE)).toBeNull();
    expect(
      screen.getAllByTestId('tier-section').every((s) => s.getAttribute('data-title') !== PLAN_TITLE),
    ).toBe(true);
  });

  it('free-floor feeds render exactly as before (no plan section, alerts intact)', () => {
    setFeed([osvAlert('vite')], 'free_floor');
    render(<PreemptionView />);
    expect(itemsInSection(VERIFIED_TITLE)).toEqual(['osv-vite']);
    expect(
      screen.getAllByTestId('tier-section').every((s) => s.getAttribute('data-title') !== PLAN_TITLE),
    ).toBe(true);
  });

  it('a plan-only feed is not the empty state', () => {
    setFeed([planStep('lodash')]);
    const { container } = render(<PreemptionView />);
    expect(within(container).queryByText('preemption.empty.title')).toBeNull();
    expect(itemsInSection(PLAN_TITLE)).toEqual(['upgrade-plan:npm:lodash']);
  });
});

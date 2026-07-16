// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// Phase 1 dependency intelligence: items with lens_hints.upgrade_plan render in
// a dedicated "Upgrade Plan" section above the tiers; the per-package OSV alert
// a plan step represents is REGROUPED out of the verified tier (same facts,
// richer framing above — never shown twice). The free floor never contains plan
// steps and is untouched.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./PreemptionFreeFloorNotice', () => ({ PreemptionFreeFloorNotice: () => <div /> }));

vi.mock('./PreemptionTierSection', () => ({
  PreemptionTierSection: ({ title, items }: { title: string; items: Array<{ id: string }> }) => (
    <div data-testid="tier-section" data-title={title}>
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
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: false },
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
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: true },
  });
}

/** The per-package OSV-verified alert for `pkg` (what the plan step regroups). */
function osvAlert(pkg: string, overrides: Record<string, unknown> = {}) {
  return baseItem(`osv-${pkg}`, { affected_deps: [pkg], ...overrides });
}

function setFeed(items: Array<ReturnType<typeof baseItem>>, tierScope: 'full' | 'free_floor' = 'full') {
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
    },
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    loadPreemption: vi.fn(),
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

  it('regroups the per-package OSV alert covered by a plan step (no duplicate card)', () => {
    setFeed([planStep('lodash'), osvAlert('lodash'), osvAlert('axios')]);
    render(<PreemptionView />);

    // lodash appears ONCE — as the plan step; its raw alert is regrouped away.
    expect(itemsInSection(PLAN_TITLE)).toEqual(['upgrade-plan:npm:lodash']);
    const verified = itemsInSection(VERIFIED_TITLE);
    expect(verified).toContain('osv-axios');
    expect(verified).not.toContain('osv-lodash');
  });

  it('never regroups non-OSV items or alerts not fully covered by the plan', () => {
    setFeed([
      planStep('lodash'),
      // Covers lodash AND axios — axios has no plan step, so this alert must stay.
      osvAlert('multi', { affected_deps: ['lodash', 'axios'] }),
      // Heuristic (non-OSV) item mentioning lodash — never regrouped.
      baseItem('chain-lodash', {
        confidence: { value: 0.5, provenance: 'heuristic', sample_size: null },
        affected_deps: ['lodash'],
      }),
    ]);
    render(<PreemptionView />);

    expect(itemsInSection(VERIFIED_TITLE)).toContain('osv-multi');
    const all = screen.getAllByTestId('tier-item').map((n) => n.textContent);
    expect(all).toContain('chain-lodash');
  });

  it('keeps platform-inactive alerts in the other-targets group even when covered', () => {
    setFeed([
      planStep('winapi'),
      osvAlert('winapi', {
        lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: true, upgrade_plan: false },
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

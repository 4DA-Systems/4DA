// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, within, fireEvent } from '@testing-library/react';
import { PreemptionWorklist } from './PreemptionWorklist';

// AD-054 "0 silent drops": the engine used to truncate the feed at 30 alerts,
// so on a corpus with 31+ Critical advisories every High one vanished. The
// backend now ships every finding; the worklist paints the first
// FINDINGS_VISIBLE_CAP (20) per section and puts the rest behind a visible
// "Show N more" — never fewer than it received, never a wrong count.

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { count?: number }) =>
      opts?.count !== undefined ? `${key}:${opts.count}` : key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
}));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));
vi.mock('./PreemptionFreeFloorNotice', () => ({ PreemptionFreeFloorNotice: () => <div /> }));
vi.mock('../ReportAge', () => ({ ReportAge: () => null }));
vi.mock('./PreemptionCard', () => ({
  URGENCY_ORDER: ['critical', 'high', 'medium', 'watch'],
  ItemCard: ({ item }: { item: { id: string } }) => <div data-testid="finding-card">{item.id}</div>,
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function osvAlert(id: string, urgency: string) {
  return {
    id,
    kind: 'alert',
    title: `item ${id}`,
    explanation: '',
    confidence: { value: 0.9, provenance: 'osv_verified', sample_size: null },
    urgency,
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
  };
}

/** 31 Critical + 14 High OSV-verified findings: the fixture shape the cap broke. */
function setLargeFeed() {
  const items = [
    ...Array.from({ length: 31 }, (_, i) => osvAlert(`crit-${i}`, 'critical')),
    ...Array.from({ length: 14 }, (_, i) => osvAlert(`high-${i}`, 'high')),
  ];
  mockState = {
    preemptionFeed: {
      items,
      total: items.length,
      critical_count: 31,
      high_count: 14,
      score: null,
      total_tracked: null,
      weak_match_count: null,
      data_freshness: null,
      tier_scope: 'full',
    },
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    preemptionLastDismissed: null,
    loadPreemption: vi.fn(),
    refreshPreemptionQuietly: vi.fn(),
    dismissPreemptionItem: vi.fn(),
    undoPreemptionDismissal: vi.fn(),
    clearPreemptionUndo: vi.fn(),
    expandPreemptionPlan: vi.fn(),
  };
}

describe('PreemptionWorklist — more findings than fit on screen', () => {
  it('counts every finding in the header bar and the section subtitle', () => {
    setLargeFeed();
    const { container } = render(<PreemptionWorklist />);
    expect(container.textContent).toContain('31 preemption.urgency.critical');
    expect(container.textContent).toContain('14 preemption.urgency.high');
    expect(container.textContent).toContain('preemption.header.onList:45');
    expect(container.textContent).not.toContain('preemption.header.setAside');
    expect(container.textContent).toContain('preemption.tier.verifiedSubtitle:45');
  });

  it('paints the first 20 and says how many more there are', () => {
    setLargeFeed();
    render(<PreemptionWorklist />);
    const section = screen.getByRole('region', { name: 'preemption.tier.verified' });
    expect(within(section).getAllByTestId('finding-card')).toHaveLength(20);
    expect(within(section).getByRole('button', { name: 'preemption.evidence.showMore:25' })).toBeInTheDocument();
  });

  it('"Show N more" reveals every finding, including the High ones past position 30', () => {
    setLargeFeed();
    render(<PreemptionWorklist />);
    const section = screen.getByRole('region', { name: 'preemption.tier.verified' });
    fireEvent.click(within(section).getByRole('button', { name: 'preemption.evidence.showMore:25' }));
    const ids = within(section).getAllByTestId('finding-card').map(n => n.textContent);
    expect(ids).toHaveLength(45);
    expect(ids).toContain('high-13');
    expect(within(section).queryByRole('button', { name: /showMore/ })).toBeNull();
  });

  // Audit L6 (2026-10-12): "61 alerts", "49 ranked upgrades" and 37 cards
  // described one list. Every number in the header now reconciles with a
  // section: the on-list count is the sum of the section subtitles, the
  // set-aside count is the collapsed other-target group plus the dormant rows.
  it('splits the total into the worklist and what is set aside at the foot', () => {
    setLargeFeed();
    const plan = Array.from({ length: 3 }, (_, i) => {
      const a = osvAlert(`plan-${i}`, 'medium');
      return { ...a, confidence: { ...a.confidence, provenance: 'heuristic' }, lens_hints: { ...a.lens_hints, upgrade_plan: true } };
    });
    const other = Array.from({ length: 5 }, (_, i) => {
      const a = osvAlert(`other-${i}`, 'watch');
      return { ...a, lens_hints: { ...a.lens_hints, other_build_target: true } };
    });
    const dormantBase = osvAlert('dormant-0', 'watch');
    const dormant = { ...dormantBase, lens_hints: { ...dormantBase.lens_hints, dormant_notice: true } };
    const feed = mockState.preemptionFeed as { items: unknown[] };
    const items = [...plan, ...other, dormant];
    // Two plan steps held back by the list transport: `total` still counts them.
    mockState.preemptionFeed = { ...feed, items, total: items.length + 2, critical_count: 0, high_count: 0 };
    const { container } = render(<PreemptionWorklist />);
    const header = screen.getByTestId('preemption-header-counts');
    // 3 shipped + 2 held-back plan steps = the plan subtitle; nothing else on the list.
    expect(header.textContent).toContain('preemption.header.onList:5');
    expect(container.textContent).toContain('preemption.upgradePlan.subtitle:5');
    // 5 other-target advisories + 1 dormant row.
    expect(header.textContent).toContain('preemption.header.setAside:6');
    expect(container.textContent).toContain('preemption.otherTargets.show:5');
    // The "N verified" chip duplicated a section subtitle and ignored the
    // (equally version-confirmed) plan steps: cut.
    expect(container.textContent).not.toContain('preemption.badge.verified');
  });
});

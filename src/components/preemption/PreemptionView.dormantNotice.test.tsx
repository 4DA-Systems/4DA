// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// 2026-09-07 audit: a dormant repo the user still owns was invisible here (its
// lockfile fell below the relevance floor), and the fix must not swing to N
// alarming rows about a repo nobody is deploying. One quiet footer row per
// dormant project, kept OUT of the urgency tiers.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));

vi.mock('./PreemptionTierSection', () => ({
  PreemptionTierSection: ({ title, items }: { title: string; items: Array<{ id: string }> }) => (
    <div data-testid="tier-section" data-title={title}>
      {items.map((i) => <span key={i.id} data-testid="tier-item">{i.id}</span>)}
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

type Hints = { other_build_target?: boolean; lockfile_only?: boolean; dormant_notice?: boolean };

function item(id: string, title: string, urgency: string, hints: Hints) {
  return {
    id,
    kind: 'alert',
    title,
    explanation: 'why',
    // osv_verified deliberately: a dormant notice inherits the provenance of
    // the matches it summarises, so the tier branches would claim it if the
    // dormant check did not come first.
    confidence: { value: 0.9, provenance: 'osv_verified', sample_size: null },
    urgency,
    reversibility: null,
    evidence: [],
    affected_projects: [],
    affected_deps: [id],
    suggested_actions: [],
    precedents: [],
    refutation_condition: null,
    lens_hints: {
      briefing: false,
      preemption: true,
      blind_spots: false,
      evidence: false,
      other_build_target: false,
      upgrade_plan: false,
      no_coverage: false,
      ...hints,
    },
    created_at: 0,
    expires_at: null,
  };
}

function setFeed(items: ReturnType<typeof item>[]) {
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
      tier_scope: 'full',
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

describe('PreemptionView — dormant-project notices', () => {
  it('renders one quiet footer row and keeps it out of the urgency tiers', () => {
    setFeed([
      item('axios', 'CVE in axios', 'high', {}),
      item('dormant-notice:/dev/navcal', 'navcal — dormant 290 days, 31 known vulnerable packages not audited', 'watch', {
        dormant_notice: true,
      }),
    ]);
    render(<PreemptionView />);

    const notices = screen.getByTestId('dormant-notices');
    expect(notices).toHaveTextContent('navcal');
    expect(notices).toHaveTextContent('31 known vulnerable packages');

    // The negative half: today's work is untouched, and the notice never
    // competes with it inside a tier.
    const tierItems = screen.getAllByTestId('tier-item').map((n) => n.textContent);
    expect(tierItems).toContain('axios');
    expect(tierItems).not.toContain('dormant-notice:/dev/navcal');
  });

  it('renders no footer at all when no project is dormant', () => {
    // Cold-start rule: no "nothing to report" rows.
    setFeed([item('axios', 'CVE in axios', 'high', {})]);
    render(<PreemptionView />);
    expect(screen.queryByTestId('dormant-notices')).toBeNull();
  });

  it('does not put a dormant notice in the other-build-targets group', () => {
    setFeed([
      item('dormant-notice:/dev/navcal', 'navcal — dormant 290 days, 2 known vulnerable packages not audited', 'watch', {
        dormant_notice: true,
      }),
    ]);
    render(<PreemptionView />);
    expect(screen.getByTestId('dormant-notices')).toBeInTheDocument();
    expect(screen.queryByText('preemption.otherTargets.show')).toBeNull();
  });
});

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// AB-011 render guard: the slice classifies the Signal gate into a paywalled
// flag (covered by preemption-slice.test.ts); THIS verifies the view actually
// renders the upgrade CTA for that flag — and NOT the red error banner. The
// slice test proves the wiring; this proves the JSX.
//
// Tier rebalance (2026-06-12): the backend no longer gates
// get_preemption_alerts — free tier receives the OSV-verified floor with
// feed.tier_scope === 'free_floor'. The paywalled branch survives only as a
// stale-backend fallback. The free-floor block below pins the new contract:
// real Tier 1 alerts render normally, locked tiers collapse to a compact
// inline notice, and the full-page lock screen never appears over real data.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({
  SignalUpgradeCTA: () => <div data-testid="signal-upgrade-cta" />,
}));
vi.mock('./PreemptionTierSection', () => ({
  PreemptionTierSection: ({ title }: { title: string }) => (
    <div data-testid="tier-section">{title}</div>
  ),
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
    loadPreemption: vi.fn(),
    ...overrides,
  };
}

describe('PreemptionView — paywall render', () => {
  it('renders the upgrade CTA + lock copy when paywalled, NOT an error banner', () => {
    setState({ preemptionPaywalled: true });
    render(<PreemptionView />);

    // localized lock copy (i18n test setup returns the key)
    expect(screen.getByText('preemption.locked.title')).toBeInTheDocument();
    expect(screen.getByText('preemption.locked.subtitle')).toBeInTheDocument();
    // the actual upgrade CTA is rendered
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
  });

  it('does NOT render a red error banner in the paywalled state', () => {
    setState({ preemptionPaywalled: true });
    const { container } = render(<PreemptionView />);
    // the error branch uses border-red-500/30 — must be absent
    expect(container.querySelector('.border-red-500\\/30')).toBeNull();
  });

  it('renders a genuine error (not the CTA) when error is set', () => {
    setState({ preemptionError: 'database is locked' });
    render(<PreemptionView />);
    expect(screen.getByText('database is locked')).toBeInTheDocument();
    expect(screen.queryByTestId('signal-upgrade-cta')).toBeNull();
  });
});

// ─── Free security floor (tier rebalance 2026-06-12) ───────────────────────

function makeFloorItem(id: string, urgency: string) {
  return {
    id,
    kind: 'alert',
    title: `CVE in ${id}`,
    explanation: 'OSV-verified version-range match',
    confidence: { value: 0.9, provenance: 'osv_verified', sample_size: null },
    urgency,
    reversibility: null,
    evidence: [],
    affected_projects: [],
    affected_deps: [id],
    suggested_actions: [],
    precedents: [],
    refutation_condition: null,
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false },
    created_at: 0,
    expires_at: null,
  };
}

function makeFloorFeed(items: ReturnType<typeof makeFloorItem>[]) {
  return {
    items,
    total: items.length,
    critical_count: items.filter((i) => i.urgency === 'critical').length,
    high_count: items.filter((i) => i.urgency === 'high').length,
    score: null,
    total_tracked: null,
    weak_match_count: null,
    data_freshness: null,
    tier_scope: 'free_floor',
  };
}

describe('PreemptionView — free floor render', () => {
  it('renders Tier 1 alerts normally with NO lock screen for a free-floor feed', () => {
    setState({ preemptionFeed: makeFloorFeed([makeFloorItem('axios', 'high')]) });
    render(<PreemptionView />);

    // The verified tier section renders the real data.
    expect(screen.getByTestId('tier-section')).toHaveTextContent('preemption.tier.verified');
    // The full-page paywall copy must NOT appear over real security data.
    expect(screen.queryByText('preemption.locked.title')).toBeNull();
    expect(screen.queryByText('preemption.locked.subtitle')).toBeNull();
  });

  it('shows the compact locked-tiers notice with an upgrade CTA on the free floor', () => {
    setState({ preemptionFeed: makeFloorFeed([makeFloorItem('axios', 'high')]) });
    render(<PreemptionView />);

    expect(screen.getByText('preemption.freeFloor.title')).toBeInTheDocument();
    expect(screen.getByText('preemption.freeFloor.subtitle')).toBeInTheDocument();
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
  });

  it('still shows the notice when the floor is empty (locked tiers exist)', () => {
    setState({ preemptionFeed: makeFloorFeed([]) });
    render(<PreemptionView />);

    // Honest empty state for the verified tier...
    expect(screen.getByText('preemption.empty.title')).toBeInTheDocument();
    // ...plus the locked-tiers notice, never the full-page lock.
    expect(screen.getByText('preemption.freeFloor.title')).toBeInTheDocument();
    expect(screen.queryByText('preemption.locked.title')).toBeNull();
  });

  it('does NOT show the free-floor notice on a full (Signal/trial) feed', () => {
    const feed = { ...makeFloorFeed([makeFloorItem('axios', 'high')]), tier_scope: 'full' };
    setState({ preemptionFeed: feed });
    render(<PreemptionView />);

    expect(screen.getByTestId('tier-section')).toBeInTheDocument();
    expect(screen.queryByText('preemption.freeFloor.title')).toBeNull();
  });
});

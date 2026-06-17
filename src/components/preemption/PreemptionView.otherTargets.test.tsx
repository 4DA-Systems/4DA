// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// Phase 2c: advisories relevant only to a build target the user does not build
// on the host (lens_hints.other_build_target) are pulled OUT of the main tiers
// into a collapsed "other build targets" group — surfaced, never hidden.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));

// Render each tier section's item ids so we can assert what landed in the tiers.
vi.mock('./PreemptionTierSection', () => ({
  PreemptionTierSection: ({ title, items }: { title: string; items: Array<{ id: string }> }) => (
    <div data-testid="tier-section" data-title={title}>
      {items.map((i) => <span key={i.id} data-testid="tier-item">{i.id}</span>)}
    </div>
  ),
}));

// Stub the card so the collapsed group renders a simple, assertable marker.
vi.mock('./PreemptionCard', () => ({
  URGENCY_ORDER: ['critical', 'high', 'medium', 'watch'],
  ItemCard: ({ item }: { item: { id: string } }) => <div data-testid="other-card">{item.id}</div>,
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function item(id: string, urgency: string, otherBuildTarget: boolean) {
  return {
    id,
    kind: 'alert',
    title: `CVE in ${id}`,
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
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: otherBuildTarget },
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
    loadPreemption: vi.fn(),
  };
}

describe('PreemptionView — other-build-targets grouping (Phase 2c)', () => {
  it('keeps platform-inactive advisories OUT of the main tiers', () => {
    setFeed([item('axios', 'high', false), item('libc', 'watch', true)]);
    render(<PreemptionView />);

    const tierItems = screen.getAllByTestId('tier-item').map((n) => n.textContent);
    expect(tierItems).toContain('axios');
    expect(tierItems).not.toContain('libc');
  });

  it('shows a collapsed group header with the count, hidden until expanded', () => {
    setFeed([item('axios', 'high', false), item('libc', 'watch', true)]);
    render(<PreemptionView />);

    // The group header (count copy) is present; the card is not rendered yet.
    expect(screen.getByText('preemption.otherTargets.show')).toBeInTheDocument();
    expect(screen.queryByTestId('other-card')).toBeNull();

    // Expanding reveals the platform-inactive item.
    fireEvent.click(screen.getByText('preemption.otherTargets.show'));
    expect(screen.getByTestId('other-card')).toHaveTextContent('libc');
  });

  it('renders no group when nothing is platform-inactive', () => {
    setFeed([item('axios', 'high', false)]);
    render(<PreemptionView />);
    expect(screen.queryByText('preemption.otherTargets.show')).toBeNull();
  });

  it('counts the platform-inactive item toward the visible total (not dropped)', () => {
    setFeed([item('libc', 'watch', true)]);
    const { container } = render(<PreemptionView />);
    // totalVisible includes the other-target item, so the view is not the
    // "your stack is clean" empty state.
    expect(within(container).queryByText('preemption.empty.title')).toBeNull();
    expect(screen.getByText('preemption.otherTargets.show')).toBeInTheDocument();
  });
});

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

let mockIsPro = true;
vi.mock('../../hooks/use-license', () => ({
  useLicense: () => ({ isPro: mockIsPro, trialStatus: null, expired: false, daysRemaining: 30 }),
}));

// AD-035: the panel reads briefVerdicts (the latest briefing's filter
// verdicts) from the store via useActiveBriefFilteredIds.
let mockBriefVerdicts: { filtered: Record<number, string>; expiresAtMs: number } | null = null;

vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
      const mockState: Record<string, unknown> = {
        startTrial: vi.fn(),
        briefVerdicts: mockBriefVerdicts,
      };
      return selector(mockState);
    }),
    { getState: () => ({}) },
  ),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { SignalsPanel } from '../SignalsPanel';
import { makeItem } from '../../test/factories';
import type { SourceRelevance } from '../../types';

function makeSignalItem(overrides: Partial<SourceRelevance> = {}) {
  return makeItem({
    signal_type: 'security_alert',
    signal_priority: 'alert',
    signal_action: 'Update dependency immediately',
    signal_triggers: ['CVE-2025-001'],
    // Distinct stories get distinct URLs (makeItem's shared default URL would
    // trip the panel's one-story-one-row dedup for unrelated fixtures).
    url: `https://example.com/article-${overrides.id ?? 1}`,
    ...overrides,
  });
}

describe('SignalsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsPro = true;
    mockBriefVerdicts = null;
  });

  it('renders nothing when there are no results', () => {
    // Hide-when-empty: an empty run must not leave a bordered "no signals" card
    // behind — the panel simply does not appear (same contract as
    // WhatYouWouldHaveMissed).
    const { container } = render(<SignalsPanel results={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when no results have signal fields', () => {
    // Items without signal_type/signal_priority/signal_action are filtered out
    const { container } = render(<SignalsPanel results={[makeItem()]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders signal items when results have signal data', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Patch this vulnerability' }),
        ]}
      />,
    );
    expect(screen.getByText('Patch this vulnerability')).toBeInTheDocument();
  });

  it('shows the signals title header', () => {
    render(
      <SignalsPanel results={[makeSignalItem({ id: 1 })]} />,
    );
    expect(screen.getByText('signals.title')).toBeInTheDocument();
  });

  it('shows signal count in subtitle', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1 }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Monitor' }),
        ]}
      />,
    );
    expect(screen.getByText('signals.actionable')).toBeInTheDocument();
  });

  it('leads the header with an "affecting you" count when a grounded signal exists', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_priority: 'critical',
            signal_action: 'Emergency patch',
            // Grounded: a strong, non-ambiguous edge to the user's dependency.
            score_breakdown: { matched_deps: ['react'], strongly_grounded: true } as never,
          }),
        ]}
      />,
    );
    expect(screen.getByText('signals.affectsYouCount')).toBeInTheDocument();
  });

  it('does NOT surface a raw critical badge for an ungrounded critical signal', () => {
    // The core fix: a critical-priority signal with no tie to the user's stack
    // must not scream "critical" in the header. It is routed to the Ambient pool
    // and the header leads with grounded ("affecting you") counts only.
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_priority: 'critical',
            signal_action: 'Some industry CVE in the news',
            // Ungrounded: no matched_deps, low domain relevance.
            score_breakdown: { matched_deps: [], domain_relevance: 0.15 } as never,
          }),
        ]}
      />,
    );
    expect(screen.queryByText('signals.critical')).not.toBeInTheDocument();
    expect(screen.queryByText('signals.affectsYouCount')).not.toBeInTheDocument();
    // It still renders as a signal, just in the de-emphasized Ambient pool.
    expect(screen.getByText('Some industry CVE in the news')).toBeInTheDocument();
  });

  it('routes a grounded critical into the Affects You pool', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_priority: 'critical',
            signal_action: 'CVE in your installed axios',
            score_breakdown: { matched_deps: ['axios'], strongly_grounded: true } as never,
          }),
        ]}
      />,
    );
    expect(screen.getByText('signals.poolAffectsYou')).toBeInTheDocument();
  });

  it('sorts signals by priority (critical first)', () => {
    const { container } = render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_priority: 'watch', signal_action: 'Low priority item' }),
          makeSignalItem({ id: 2, signal_priority: 'critical', signal_action: 'Critical item' }),
          makeSignalItem({ id: 3, signal_priority: 'alert', signal_action: 'High priority item' }),
        ]}
      />,
    );
    // Get all signal action texts in order
    const actions = container.querySelectorAll('.text-sm.font-medium');
    const texts = Array.from(actions).map((el) => el.textContent);
    expect(texts[0]).toBe('Critical item');
    expect(texts[1]).toBe('High priority item');
    expect(texts[2]).toBe('Low priority item');
  });

  it('collapses panel when header is clicked', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Visible action' }),
        ]}
      />,
    );

    // Initially expanded
    expect(screen.getByText('Visible action')).toBeInTheDocument();

    // Click header to collapse
    fireEvent.click(screen.getByRole('button', { name: /signals\.title/ }));

    // Signal content should be hidden
    expect(screen.queryByText('Visible action')).not.toBeInTheDocument();
  });

  it('re-expands panel when header is clicked again', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Toggle action' }),
        ]}
      />,
    );

    // Collapse
    fireEvent.click(screen.getByRole('button', { name: /signals\.title/ }));
    expect(screen.queryByText('Toggle action')).not.toBeInTheDocument();

    // Expand
    fireEvent.click(screen.getByRole('button', { name: /signals\.title/ }));
    expect(screen.getByText('Toggle action')).toBeInTheDocument();
  });

  it('shows type filter buttons for each signal type', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Watch trend' }),
        ]}
      />,
    );

    // "Security" appears in both filter and signal row badge, so use getAllByText
    expect(screen.getAllByText('Security').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Trends').length).toBeGreaterThanOrEqual(1);
  });

  it('filters by type when type filter button is clicked', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Watch trend' }),
        ]}
      />,
    );

    // Click the Security filter button (it has a count child element).
    // The filter buttons are in the filter bar; find the first "Security" that is inside a button.
    const securityElements = screen.getAllByText('Security');
    const filterBtn = securityElements.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    expect(filterBtn).toBeTruthy();
    fireEvent.click(filterBtn!);

    // Only security items should be visible
    expect(screen.getByText('Patch vuln')).toBeInTheDocument();
    expect(screen.queryByText('Watch trend')).not.toBeInTheDocument();
  });

  it('clears type filter when clicking active filter button', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Watch trend' }),
        ]}
      />,
    );

    // Find and click the Security filter button
    const getFilterBtn = () => {
      const els = screen.getAllByText('Security');
      return els.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    };

    // Activate filter
    fireEvent.click(getFilterBtn()!);
    expect(screen.queryByText('Watch trend')).not.toBeInTheDocument();

    // Deactivate filter
    fireEvent.click(getFilterBtn()!);
    expect(screen.getByText('Watch trend')).toBeInTheDocument();
  });

  it('shows "clear" button when filters are active', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Trend' }),
        ]}
      />,
    );

    // No clear button initially
    expect(screen.queryByText('signals.clear')).not.toBeInTheDocument();

    // Find and click the Security filter button
    const securityElements = screen.getAllByText('Security');
    const filterBtn = securityElements.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    fireEvent.click(filterBtn!);

    // Clear button should appear
    expect(screen.getByText('signals.clear')).toBeInTheDocument();
  });

  it('clears all filters when clear button is clicked', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert', signal_action: 'Patch vuln' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Trend signal' }),
        ]}
      />,
    );

    // Find and click the Security filter button
    const securityElements = screen.getAllByText('Security');
    const filterBtn = securityElements.find((el) => el.closest('button[class*="rounded-lg"]'))?.closest('button');
    fireEvent.click(filterBtn!);
    expect(screen.queryByText('Trend signal')).not.toBeInTheDocument();

    // Click clear
    fireEvent.click(screen.getByText('signals.clear'));

    // All items should be visible again
    expect(screen.getByText('Patch vuln')).toBeInTheDocument();
    expect(screen.getByText('Trend signal')).toBeInTheDocument();
  });

  it('shows trigger toggle button when signal has triggers', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_triggers: ['CVE-2025-001', 'dependency-update'],
          }),
        ]}
      />,
    );

    expect(screen.getByText('signals.showTriggers')).toBeInTheDocument();
  });

  it('shows similar items count when signal has similar items', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            similar_count: 3,
            similar_titles: ['Similar Item A', 'Similar Item B'],
          }),
        ]}
      />,
    );

    expect(screen.getByText(/signals\.similar/)).toBeInTheDocument();
  });

  // ===========================================================================
  // One story, one row — URL-level dedup (live audit 2026-08-31)
  // ===========================================================================

  it('collapses same-URL signals into a single row', () => {
    // The audit's exact shape: one URL, three ALERT rows (HN, Lobsters, HN),
    // accumulated across differential cycles under different item ids.
    const url = 'https://blog.wybxc.cc/blog/rust-gui-survey-2026/';
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, url, source_type: 'hackernews', signal_action: 'Row one' }),
          makeSignalItem({ id: 2, url: 'https://www.blog.wybxc.cc/blog/rust-gui-survey-2026', source_type: 'lobsters', signal_action: 'Row two' }),
          makeSignalItem({ id: 3, url, source_type: 'hackernews', signal_action: 'Row three' }),
        ]}
      />,
    );
    const rows = screen.getAllByText(/^Row (one|two|three)$/);
    expect(rows).toHaveLength(1);
    // The header count reflects the deduped list, not the raw row count.
    expect(screen.getByText('signals.actionable')).toBeInTheDocument();
  });

  it('keeps the highest-priority copy when the same URL appears twice', () => {
    const url = 'https://example.com/one-story';
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, url, signal_priority: 'advisory', signal_action: 'Advisory copy' }),
          makeSignalItem({ id: 2, url, signal_priority: 'critical', signal_action: 'Critical copy' }),
        ]}
      />,
    );
    expect(screen.getByText('Critical copy')).toBeInTheDocument();
    expect(screen.queryByText('Advisory copy')).not.toBeInTheDocument();
  });

  it('never collapses distinct URLs or items without a URL', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, url: 'https://example.com/a', signal_action: 'Story A' }),
          makeSignalItem({ id: 2, url: 'https://example.com/b', signal_action: 'Story B' }),
          makeSignalItem({ id: 3, url: null, signal_action: 'No URL one' }),
          makeSignalItem({ id: 4, url: null, signal_action: 'No URL two' }),
        ]}
      />,
    );
    expect(screen.getByText('Story A')).toBeInTheDocument();
    expect(screen.getByText('Story B')).toBeInTheDocument();
    expect(screen.getByText('No URL one')).toBeInTheDocument();
    expect(screen.getByText('No URL two')).toBeInTheDocument();
  });

  // ===========================================================================
  // Grounding chip / card copy coherence (live audit 2026-08-31)
  // ===========================================================================

  it('renders the dependency chip only for grounded (Affects You) signals', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_action: 'Grounded tool signal',
            score_breakdown: { matched_deps: ['tokio'], strongly_grounded: true } as never,
          }),
        ]}
      />,
    );
    expect(screen.getByText(/tokio/)).toBeInTheDocument();
  });

  it('does NOT render the dependency chip when matched_deps is only a weak, ungrounded hit', () => {
    // matched_deps can carry bare subterm hits (e.g. "windows" from
    // windows-sys) that are NOT real grounding. A card whose copy says
    // "no confirmed link" must not simultaneously flash a green
    // "Matches your dependencies" chip.
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_action: 'New tool spotted — no confirmed link to your stack',
            score_breakdown: {
              matched_deps: ['tokio'],
              strongly_grounded: false,
              domain_relevance: 0.15,
            } as never,
          }),
        ]}
      />,
    );
    expect(screen.getByText('New tool spotted — no confirmed link to your stack')).toBeInTheDocument();
    expect(screen.queryByText(/🎯/)).not.toBeInTheDocument();
  });
});

// =============================================================================
// Free Tier Behavior
// =============================================================================
describe('SignalsPanel (free tier)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsPro = false;
  });

  it('does not render signal action items when isPro is false', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Patch this vulnerability' }),
        ]}
      />,
    );
    expect(screen.queryByText('Patch this vulnerability')).not.toBeInTheDocument();
  });

  it('shows upgrade CTA text when isPro is false', () => {
    render(
      <SignalsPanel
        results={[makeSignalItem({ id: 1 })]}
      />,
    );
    expect(screen.getByText('pro.upgrade')).toBeInTheDocument();
  });

  it('shows free teaser text when isPro is false', () => {
    render(
      <SignalsPanel
        results={[makeSignalItem({ id: 1 })]}
      />,
    );
    expect(screen.getByText(/signals\.freeTeaser/)).toBeInTheDocument();
  });

  it('shows category pills as read-only spans (not buttons) in free tier', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_type: 'security_alert' }),
          makeSignalItem({ id: 2, signal_type: 'tech_trend', signal_priority: 'advisory', signal_action: 'Watch trend' }),
        ]}
      />,
    );
    // Category labels should be visible
    expect(screen.getAllByText('Security').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Trends').length).toBeGreaterThanOrEqual(1);

    // They should be rendered as spans, not filter buttons
    const securityEls = screen.getAllByText('Security');
    const isInsideButton = securityEls.some((el) => el.closest('button[class*="rounded-lg"]'));
    expect(isInsideButton).toBe(false);
  });

  it('still renders the panel header and allows collapse in free tier', () => {
    render(
      <SignalsPanel
        results={[makeSignalItem({ id: 1, signal_action: 'Some action' })]}
      />,
    );

    // Header is visible
    expect(screen.getByText('signals.title')).toBeInTheDocument();

    // Collapse should hide the teaser content
    fireEvent.click(screen.getByRole('button', { name: /signals\.title/ }));
    expect(screen.queryByText(/signals\.freeTeaser/)).not.toBeInTheDocument();
    expect(screen.queryByText('pro.upgrade')).not.toBeInTheDocument();
  });

  it('renders nothing when no signals exist in free tier', () => {
    // An empty container also means no upgrade CTA can be shown on emptiness.
    const { container } = render(<SignalsPanel results={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  // ---------------------------------------------------------------------------
  // AD-035: one item, one verdict — the latest briefing's filter verdicts
  // demote items out of the Key Signals lane while the briefing is fresh.
  // ---------------------------------------------------------------------------
  describe('briefing verdict binding (AD-035)', () => {
    beforeEach(() => {
      // This block sits inside the free-tier describe (mockIsPro = false);
      // the verdict-binding assertions read Pro-tier signal rows.
      mockIsPro = true;
    });

    it('demotes a Key Signal the briefing filtered and shows the suppressed count', () => {
      // The live-audit contradiction: the briefing filtered an item as
      // self-promotion while the panel promoted it as an ALERT.
      mockBriefVerdicts = {
        filtered: { 1: 'self-promotional' },
        expiresAtMs: Date.now() + 60_000,
      };
      render(
        <SignalsPanel
          results={[
            makeSignalItem({ id: 1, signal_action: 'Promoted self-promo action' }),
            makeSignalItem({ id: 2, signal_action: 'Legit alert action' }),
          ]}
        />,
      );
      expect(screen.queryByText('Promoted self-promo action')).not.toBeInTheDocument();
      expect(screen.getByText('Legit alert action')).toBeInTheDocument();
      // Suppression is observable: the header carries a count.
      expect(screen.getByTestId('brief-suppressed-count')).toBeInTheDocument();
    });

    it('never suppresses deterministic security truth (is_critical_alert)', () => {
      mockBriefVerdicts = {
        filtered: { 1: 'noise' },
        expiresAtMs: Date.now() + 60_000,
      };
      render(
        <SignalsPanel
          results={[
            makeSignalItem({ id: 1, is_critical_alert: true, signal_action: 'Confirmed CVE action' }),
          ]}
        />,
      );
      expect(screen.getByText('Confirmed CVE action')).toBeInTheDocument();
      expect(screen.queryByTestId('brief-suppressed-count')).not.toBeInTheDocument();
    });

    it('expired verdicts bind nothing — the item renders exactly as today', () => {
      mockBriefVerdicts = {
        filtered: { 1: 'noise' },
        expiresAtMs: Date.now() - 1,
      };
      render(
        <SignalsPanel
          results={[makeSignalItem({ id: 1, signal_action: 'Back after expiry' })]}
        />,
      );
      expect(screen.getByText('Back after expiry')).toBeInTheDocument();
      expect(screen.queryByTestId('brief-suppressed-count')).not.toBeInTheDocument();
    });
  });
});

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

// Pass the key through, suffixed with `count` when given, so header / teaser /
// toggle counts are observable ("signals.actionable:2").
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { count?: number }) =>
      opts && typeof opts.count === 'number' ? `${key}:${opts.count}` : key,
    i18n: { language: 'en', changeLanguage: vi.fn() },
  }),
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
import { useSignalDisplayOrder } from '../signals/signal-display-order';
import { makeItem } from '../../test/factories';
import type { SourceRelevance } from '../../types';

// Default fixture is grounded (Affects You) — the only pool shown by default.
// Grounded without matched_deps so no dependency chip renders.
function makeSignalItem(overrides: Partial<SourceRelevance> = {}) {
  return makeItem({
    signal_type: 'security_alert',
    signal_priority: 'alert',
    signal_action: 'Update dependency immediately',
    signal_triggers: ['CVE-2025-001'],
    score_breakdown: { matched_deps: [], strongly_grounded: true } as never,
    // Distinct stories get distinct URLs (makeItem's shared default URL would
    // trip the panel's one-story-one-row dedup for unrelated fixtures).
    url: `https://example.com/article-${overrides.id ?? 1}`,
    ...overrides,
  });
}

/** An ungrounded, on-stack item: lands in the In Your Orbit pool. */
function makeOrbitItem(overrides: Partial<SourceRelevance> = {}) {
  return makeSignalItem({
    signal_type: 'tech_trend',
    signal_priority: 'advisory',
    score_breakdown: { matched_deps: [], domain_relevance: 0.85 } as never,
    ...overrides,
  });
}

const visibleText = { ignore: 'script, style, [aria-hidden="true"]' };
const clickOrbitToggle = () => fireEvent.click(screen.getByTestId('signals-orbit-toggle'));

beforeEach(() => {
  useSignalDisplayOrder.setState({ orbitExpanded: false });
});

describe('SignalsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockIsPro = true;
    mockBriefVerdicts = null;
  });

  // ===========================================================================
  // Affects You is the default view (audit 2026-10-07, Decisions 3/6)
  // ===========================================================================

  it('renders only Affects You rows by default', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Grounded row' }),
          makeOrbitItem({ id: 2, signal_action: 'Orbit row A' }),
          makeOrbitItem({ id: 3, signal_action: 'Orbit row B' }),
        ]}
      />,
    );
    expect(screen.getByText('Grounded row')).toBeInTheDocument();
    expect(screen.queryByText('Orbit row A')).not.toBeInTheDocument();
    expect(screen.queryByText('Orbit row B')).not.toBeInTheDocument();
    expect(screen.queryByText('signals.poolInOrbit')).not.toBeInTheDocument();
    const toggle = screen.getByTestId('signals-orbit-toggle');
    expect(toggle).toHaveTextContent('signals.orbitShow');
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
  });

  it('reveals orbit rows when the toggle is clicked, and hides them again', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Grounded row' }),
          makeOrbitItem({ id: 2, signal_action: 'Orbit row' }),
        ]}
      />,
    );
    clickOrbitToggle();
    expect(screen.getByText('Orbit row')).toBeInTheDocument();
    expect(screen.getByText('signals.poolInOrbit')).toBeInTheDocument();
    expect(screen.getByTestId('signals-orbit-toggle')).toHaveTextContent('signals.orbitHide');
    expect(useSignalDisplayOrder.getState().orbitExpanded).toBe(true);

    clickOrbitToggle();
    expect(screen.queryByText('Orbit row')).not.toBeInTheDocument();
  });

  it('hides the whole panel when nothing affects you', () => {
    const { container } = render(
      <SignalsPanel
        results={[
          makeOrbitItem({ id: 1, signal_action: 'Generic news' }),
          makeOrbitItem({ id: 2, signal_action: 'More generic news' }),
        ]}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('omits the toggle when there is nothing outside Affects You', () => {
    render(<SignalsPanel results={[makeSignalItem({ id: 1 })]} />);
    expect(screen.queryByTestId('signals-orbit-toggle')).not.toBeInTheDocument();
  });

  it('filter chips count the default (Affects You) view only', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1 }),
          makeSignalItem({ id: 2 }),
          makeOrbitItem({ id: 3 }),
          makeOrbitItem({ id: 4 }),
          makeOrbitItem({ id: 5 }),
        ]}
      />,
    );
    // Header and toggle counts read the default view.
    expect(screen.getByText('signals.actionable:2')).toBeInTheDocument();
    expect(screen.getByTestId('signals-orbit-toggle')).toHaveTextContent('signals.orbitShow:3');
    const chip = (label: string) =>
      screen.queryAllByText(label).find((el) => el.closest('button[aria-pressed]'))?.closest('button');
    expect(chip('Security')).toHaveTextContent('2');
    expect(chip('Trends')).toBeUndefined();
    // No advisory priority in the default view (all three advisories are orbit).
    expect(screen.queryByRole('button', { name: /Filter by priority: .*advis/i })).not.toBeInTheDocument();

    clickOrbitToggle();
    expect(chip('Trends')).toHaveTextContent('3');
    expect(screen.getByText('signals.actionable:5')).toBeInTheDocument();
    // Expanded, the header distinguishes the grounded subset.
    expect(screen.getByText('signals.affectsYouCount:2')).toBeInTheDocument();
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

  it('keeps pipeline-rejected items out of Key Signals, except critical alerts', () => {
    // Demotion clears `relevant` but keeps the signal fields (live audit
    // 2026-10-02: 48 of 104 Key Signals were rejected items).
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Kept: surfaced item' }),
          makeSignalItem({ id: 2, relevant: false, excluded: true, signal_action: 'Gone: judge-rejected' }),
          makeSignalItem({ id: 3, excluded: true, signal_action: 'Gone: excluded' }),
          makeSignalItem({ id: 4, relevant: false, is_critical_alert: true, signal_action: 'Kept: critical alert' }),
        ]}
      />,
    );
    expect(screen.getByText('Kept: surfaced item')).toBeInTheDocument();
    expect(screen.getByText('Kept: critical alert')).toBeInTheDocument();
    expect(screen.queryByText('Gone: judge-rejected')).not.toBeInTheDocument();
    expect(screen.queryByText('Gone: excluded')).not.toBeInTheDocument();
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
    expect(screen.getByText('signals.actionable:2')).toBeInTheDocument();
  });

  it('does NOT surface a raw critical badge for an ungrounded critical signal', () => {
    // A critical-priority signal with no tie to the user's stack must not
    // scream "critical": it is routed to the Ambient pool, hidden by default,
    // and the default chips count grounded rows only.
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_priority: 'advisory', signal_action: 'Grounded advisory' }),
          makeSignalItem({
            id: 2,
            signal_priority: 'critical',
            signal_action: 'Some industry CVE in the news',
            // Ungrounded: no matched_deps, low domain relevance.
            score_breakdown: { matched_deps: [], domain_relevance: 0.15 } as never,
          }),
        ]}
      />,
    );
    expect(screen.queryByRole('button', { name: /Filter by priority: CRITICAL/ })).not.toBeInTheDocument();
    expect(screen.queryByText('Some industry CVE in the news')).not.toBeInTheDocument();
    // Still reachable — in the de-emphasized Ambient pool behind the toggle.
    clickOrbitToggle();
    expect(screen.getByText('Some industry CVE in the news')).toBeInTheDocument();
    expect(screen.getByText('signals.poolAmbient')).toBeInTheDocument();
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
    expect(screen.getByText('signals.actionable:1')).toBeInTheDocument();
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
          makeSignalItem({ id: 1, signal_action: 'Grounded anchor' }),
          makeSignalItem({
            id: 2,
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
    clickOrbitToggle();
    expect(screen.getByText('New tool spotted — no confirmed link to your stack')).toBeInTheDocument();
    expect(screen.queryByText(/🎯/, visibleText)).not.toBeInTheDocument();
  });

  it('keeps a grounded tutorial (no dependency event) out of Affects You', () => {
    // Live 2026-10-04: "Progressive Hydration in React" was labelled
    // "🎯 react" in Affects You. It names react, but nothing is happening TO
    // react — the backend's dependency_event claim is false.
    render(
      <SignalsPanel
        results={[
          makeSignalItem({ id: 1, signal_action: 'Grounded anchor' }),
          makeSignalItem({
            id: 2,
            signal_action: 'Progressive Hydration in React',
            score_breakdown: {
              matched_deps: ['react'],
              strongly_grounded: true,
              dependency_event: false,
              domain_relevance: 0.85,
            } as never,
          }),
        ]}
      />,
    );
    // Not in the default (Affects You) view...
    expect(screen.queryByText('Progressive Hydration in React')).not.toBeInTheDocument();
    // ...it is an orbit row, with no dependency chip.
    clickOrbitToggle();
    expect(screen.getByText('Progressive Hydration in React')).toBeInTheDocument();
    expect(screen.getByText('signals.poolInOrbit')).toBeInTheDocument();
    expect(screen.getByText('signals.affectsYouCount:1')).toBeInTheDocument();
    expect(screen.queryByText(/🎯/, visibleText)).not.toBeInTheDocument();
  });

  it('admits a grounded dependency event to Affects You', () => {
    render(
      <SignalsPanel
        results={[
          makeSignalItem({
            id: 1,
            signal_action: 'Announcing Tauri 2.12',
            score_breakdown: { matched_deps: ['tauri'], strongly_grounded: true, dependency_event: true } as never,
          }),
        ]}
      />,
    );
    expect(screen.getByText('signals.poolAffectsYou')).toBeInTheDocument();
    expect(screen.getByText(/tauri/)).toBeInTheDocument();
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

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Mocks — the bar pulls in the brand mark, void signals, command search and
// several status dots; none of them are under test here.
// ---------------------------------------------------------------------------
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string | Record<string, unknown>) =>
      typeof fallback === 'string' ? fallback : key,
  }),
}));
vi.mock('../void-engine/BrandMark', () => ({
  BrandMark: ({ active }: { active?: boolean }) => (
    <div data-testid="brand-mark" data-active={String(active)} />
  ),
}));
vi.mock('../../hooks/use-void-signals', () => ({ useVoidSignals: () => 'idle' }));
const backgroundWork = vi.hoisted((): { current: boolean } => ({ current: false }));
vi.mock('../../hooks/use-background-work', () => ({ useBackgroundWork: () => backgroundWork.current }));
vi.mock('../OllamaStatus', () => ({ OllamaStatus: () => null }));
vi.mock('../SystemHealthDot', () => ({ SystemHealthDot: () => null }));
vi.mock('../ThemeToggle', () => ({ ThemeToggle: () => null }));
vi.mock('../search/CommandSearch', () => ({ CommandSearch: () => null }));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn(() => Promise.resolve()) }));
vi.mock('../../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ appState: { progress: 0 }, generateBriefing: vi.fn() }),
}));

import { UnifiedAppBar } from './UnifiedAppBar';

const UNJUDGED_TOOLTIP = 'Fast pass, LLM judge not applied yet';

function renderBar(overrides: Partial<Parameters<typeof UnifiedAppBar>[0]> = {}) {
  return render(
    <UnifiedAppBar
      state={{ loading: false, analysisComplete: true }}
      monitoring={null}
      settingsFormProvider="anthropic"
      isPro={false}
      tier="FREE"
      summaryBadges={{ relevantCount: 69, topCount: 12, total: 580 }}
      judged={false}
      aiBriefing={{ error: null }}
      onAnalyze={() => {}}
      onOpenSettings={() => {}}
      analysisPulse={false}
      {...overrides}
    />,
  );
}

describe('UnifiedAppBar — unjudged badge on the relevant chip', () => {
  it('badges an unjudged run next to the relevant count, with the fast-pass tooltip', () => {
    // The fresh-launch run is `foreground_fast` (llm_rerank: false): its "69
    // relevant" is pipeline scores only. Without the badge that reads as a
    // judged 69 — the exact confusion measured live on 2026-09-04.
    renderBar({ judged: false });
    const badge = screen.getByTestId('unjudged-badge');
    expect(badge).toHaveTextContent('unjudged');
    expect(badge).toHaveAttribute('title', UNJUDGED_TOOLTIP);
    expect(badge).toHaveAttribute('aria-label', UNJUDGED_TOOLTIP);
    // …and the count itself is still there, unchanged.
    expect(screen.getByText('69')).toBeInTheDocument();
  });

  it('drops the badge once the backend reports a judged result set', () => {
    renderBar({ judged: true });
    expect(screen.queryByTestId('unjudged-badge')).toBeNull();
    expect(screen.getByText('69')).toBeInTheDocument();
  });

  it('shows no badge while there is no chip to badge (analysis not complete)', () => {
    renderBar({ judged: false, state: { loading: true, analysisComplete: false } });
    expect(screen.queryByTestId('unjudged-badge')).toBeNull();
    expect(screen.queryByText('69')).toBeNull();
  });

  it('shows no badge without summary badges even when unjudged', () => {
    renderBar({ judged: false, summaryBadges: null });
    expect(screen.queryByTestId('unjudged-badge')).toBeNull();
  });
});

// Audit 2026-11-11: a green "Analysis complete" never aged and "LIVE" only
// meant monitoring was switched on. The bar states the real ages instead.
describe('UnifiedAppBar — freshness', () => {
  const hoursAgo = (h: number) => new Date(Date.now() - h * 3_600_000).toISOString();
  const sync = (state: 'ready' | 'failed' | 'stale', h: number | null) => ({
    state, ecosystems: 2, synced_at: h === null ? null
      : new Date(Date.now() - h * 3_600_000).toISOString().replace('T', ' ').slice(0, 19),
    error: state === 'failed' ? 'offline' : null, in_flight: false, skipped_temp: 0,
  });

  it('states the age of the last cycle and of the advisory check', () => {
    renderBar({ monitoring: { enabled: true, last_cycle_at: hoursAgo(2), advisory_sync: sync('ready', 3) } });
    const line = screen.getByTestId('header-freshness');
    expect(line).toHaveTextContent('header.freshness.updated');
    expect(line).toHaveTextContent('header.freshness.advisories');
    expect(line.getAttribute('data-amber')).toBe('false');
    expect(screen.queryByText('header.live')).toBeNull();
    expect(screen.queryByText('action.analysisComplete')).toBeNull();
  });

  // PR #1131 CI + review: one polite live region, announcing state changes
  // only; the ticking ages sit outside it.
  it('keeps one polite live status region that announces the state, not the age', () => {
    renderBar({ monitoring: { enabled: true, last_cycle_at: hoursAgo(2), advisory_sync: sync('ready', 3) } });
    const status = screen.getByRole('status');
    expect(status.getAttribute('aria-live')).toBe('polite');
    expect(status).toHaveTextContent('header.freshness.state.fresh');
    expect(status).not.toContainElement(screen.getByTestId('header-freshness'));
  });

  it('the region text does not change when only the age moves', () => {
    const { rerender } = renderBar({ monitoring: { enabled: true, last_cycle_at: hoursAgo(2), advisory_sync: sync('ready', 3) } });
    const before = screen.getByRole('status').textContent;
    const line = screen.getByTestId('header-freshness').innerHTML;
    rerender(
      <UnifiedAppBar
        state={{ loading: false, analysisComplete: true }}
        monitoring={{ enabled: true, last_cycle_at: hoursAgo(5), advisory_sync: sync('ready', 6) }}
        settingsFormProvider="anthropic"
        isPro={false}
        tier="FREE"
        summaryBadges={{ relevantCount: 69, topCount: 12, total: 580 }}
        judged={false}
        aiBriefing={{ error: null }}
        onAnalyze={() => {}}
        onOpenSettings={() => {}}
        analysisPulse={false}
      />,
    );
    expect(screen.getByRole('status').textContent).toBe(before);
    expect(screen.getByTestId('header-freshness').innerHTML).toBe(line); // keys only under the i18n mock
  });

  it('announces a state change: a failed check, then analyzing', () => {
    renderBar({ monitoring: { enabled: true, last_cycle_at: hoursAgo(1), advisory_sync: sync('failed', 2) } });
    expect(screen.getByRole('status')).toHaveTextContent('header.freshness.state.checkFailed');
  });

  it('turns amber when the last cycle is more than a day old', () => {
    renderBar({ monitoring: { enabled: true, last_cycle_at: hoursAgo(30), advisory_sync: sync('ready', 1) } });
    expect(screen.getByTestId('header-freshness').getAttribute('data-amber')).toBe('true');
  });

  it('turns amber and says so when the advisory check failed', () => {
    renderBar({ monitoring: { enabled: true, last_cycle_at: hoursAgo(1), advisory_sync: sync('failed', 2) } });
    const line = screen.getByTestId('header-freshness');
    expect(line.getAttribute('data-amber')).toBe('true');
    expect(line).toHaveTextContent('header.freshness.advisoriesFailedAsOf');
  });

  it('never reads as fresh before any cycle succeeded', () => {
    renderBar({ state: { loading: false, analysisComplete: true }, monitoring: { enabled: true, last_cycle_at: null } });
    const line = screen.getByTestId('header-freshness');
    expect(line).toHaveTextContent('header.freshness.notUpdated');
    expect(line.getAttribute('data-amber')).toBe('true');
  });
});

describe('UnifiedAppBar — the brand mark moves only while 4DA is working', () => {
  it('rests when idle: no analysis running and no background fetch', () => {
    backgroundWork.current = false;
    renderBar({ state: { loading: false, analysisComplete: true } });
    expect(screen.getByTestId('brand-mark')).toHaveAttribute('data-active', 'false');
  });

  it('animates during a foreground analysis', () => {
    backgroundWork.current = false;
    renderBar({ state: { loading: true, analysisComplete: false } });
    expect(screen.getByTestId('brand-mark')).toHaveAttribute('data-active', 'true');
  });

  it('animates during a fresh background source fetch', () => {
    backgroundWork.current = true;
    renderBar({ state: { loading: false, analysisComplete: true } });
    expect(screen.getByTestId('brand-mark')).toHaveAttribute('data-active', 'true');
    backgroundWork.current = false;
  });
});

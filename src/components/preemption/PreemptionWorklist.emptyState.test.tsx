// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PreemptionWorklist } from './PreemptionWorklist';

// Fresh-profile E2E 2026-10-10: a profile with NO dependencies was told "Your
// stack is clean — Intelligence is actively monitoring". An empty worklist is
// "no threats" only when there is a stack; with none known it asks for the
// project scan (the Blind Spots day-one flow, #888).

vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));

const cmdMock = vi.fn((..._args: unknown[]) => Promise.resolve({}));
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
  CommandTimeoutError: class CommandTimeoutError extends Error {},
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

const loadPreemption = vi.fn();
const refreshPreemptionQuietly = vi.fn(() => Promise.resolve());
const runAutoDiscovery = vi.fn(() => Promise.resolve());
const loadUserContext = vi.fn(() => Promise.resolve());

type Sync = {
  state: string; ecosystems: number; synced_at: string | null; error: string | null;
  in_flight?: boolean; skipped_temp?: number;
};
const READY: Sync = { state: 'ready', ecosystems: 2, synced_at: '2026-10-12 06:00:00', error: null };

function setFeed(
  totalTracked: number | null,
  advisorySync: Sync | null = READY,
  extra: Record<string, unknown> = {},
) {
  mockState = {
    sourceHealth: [],
    ...extra,
    preemptionFeed: {
      items: [],
      total: 0,
      critical_count: 0,
      high_count: 0,
      score: null,
      total_tracked: totalTracked,
      weak_match_count: null,
      data_freshness: null,
      tier_scope: 'full',
      advisory_sync: advisorySync,
    },
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    preemptionLastDismissed: null,
    loadPreemption,
    refreshPreemptionQuietly,
    dismissPreemptionItem: vi.fn(),
    undoPreemptionDismissal: vi.fn(),
    clearPreemptionUndo: vi.fn(),
    expandPreemptionPlan: vi.fn(),
    runAutoDiscovery,
    loadUserContext,
    isScanning: false,
    setShowSettings: vi.fn(),
    setSettingsInitialTab: vi.fn(),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('PreemptionWorklist — empty state', () => {
  it('asks for a project scan when no dependencies are known', async () => {
    setFeed(0);
    render(<PreemptionWorklist />);

    const prompt = screen.getByTestId('preemption-scan-prompt');
    expect(prompt).toHaveTextContent('preemption.noDeps.title');
    expect(screen.queryByText('preemption.empty.title')).toBeNull();

    loadPreemption.mockClear();
    fireEvent.click(screen.getByText('onboarding.choice.scanProjects'));
    await waitFor(() => expect(loadPreemption).toHaveBeenCalled());
    expect(runAutoDiscovery).toHaveBeenCalled();
  });

  it('says the stack is clean only when dependencies are known', () => {
    setFeed(166);
    render(<PreemptionWorklist />);
    expect(screen.getByText('preemption.empty.title')).toBeInTheDocument();
    expect(screen.queryByTestId('preemption-scan-prompt')).toBeNull();
  });

  it('keeps the no-threats copy when the dependency count could not be read', () => {
    setFeed(null);
    render(<PreemptionWorklist />);
    expect(screen.getByText('preemption.empty.title')).toBeInTheDocument();
    expect(screen.queryByTestId('preemption-scan-prompt')).toBeNull();
  });
});

// The all-clear is a claim that every installed version was matched against
// synced advisories. It used to wait for 100 fetched items across enabled
// sources, which a stack-only profile (AD-054) with a small project may never
// reach. Now it waits for exactly what makes it true: the advisory sync.
describe('PreemptionWorklist — the all-clear follows advisory readiness', () => {
  const CLEAN = 'preemption.empty.title';

  it('no lockfiles: the scan prompt, whatever the advisory state', () => {
    setFeed(0, { ...READY, state: 'no_inventory', ecosystems: 0 });
    render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-scan-prompt')).toBeInTheDocument();
    expect(screen.queryByText(CLEAN)).toBeNull();
    expect(screen.queryByTestId(/preemption-advisories-/)).toBeNull();
  });

  it('lockfiles read but advisories never synced: checking, not clean', () => {
    setFeed(40, { state: 'syncing', ecosystems: 2, synced_at: null, error: null });
    render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-advisories-checking')).toHaveTextContent('preemption.advisories.checking.title');
    expect(screen.queryByText(CLEAN)).toBeNull();
  });

  it('synced after this feed was computed: still checking, and asks again', () => {
    vi.useFakeTimers();
    try {
      setFeed(40, { ...READY, state: 'matching' });
      render(<PreemptionWorklist />);
      expect(screen.getByTestId('preemption-advisories-checking')).toBeInTheDocument();
      expect(screen.queryByText(CLEAN)).toBeNull();
      vi.advanceTimersByTime(30_000);
      expect(refreshPreemptionQuietly).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('the sync failed: the error, never the all-clear', () => {
    setFeed(40, { state: 'failed', ecosystems: 2, synced_at: null, error: 'npm: HTTP 503' });
    render(<PreemptionWorklist />);
    const notice = screen.getByTestId('preemption-advisories-failed');
    expect(notice).toHaveTextContent('preemption.advisories.failed.title');
    expect(notice).toHaveTextContent('npm: HTTP 503');
    expect(screen.queryByText(CLEAN)).toBeNull();
  });

  it('advisory data past the trust horizon: stale, not clean', () => {
    setFeed(40, { ...READY, state: 'stale', synced_at: '2026-10-01 06:00:00' });
    render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-advisories-stale')).toHaveTextContent('preemption.advisories.stale.title');
    expect(screen.queryByText(CLEAN)).toBeNull();
  });

  it('dependencies in no ecosystem the advisory database covers: says so', () => {
    setFeed(12, { state: 'no_inventory', ecosystems: 0, synced_at: null, error: null });
    render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-advisories-uncovered')).toBeInTheDocument();
    expect(screen.queryByText(CLEAN)).toBeNull();
  });

  // Fresh-profile audit E2/E3: a profile showed "syncing the advisory
  // database" for 8.5 h while no sync ran. Not started is its own state, and
  // the user can start the check.
  it('lockfiles read, no sync ever ran: says so and offers Check now', async () => {
    setFeed(40, { state: 'not_started', ecosystems: 2, synced_at: null, error: null, in_flight: false });
    render(<PreemptionWorklist />);
    const notice = screen.getByTestId('preemption-advisories-not_started');
    expect(notice).toHaveTextContent('preemption.advisories.notStarted.title');
    expect(screen.queryByText(CLEAN)).toBeNull();
    fireEvent.click(screen.getByTestId('preemption-advisories-check-now'));
    expect(cmdMock).toHaveBeenCalledWith('osv_sync_now');
    await waitFor(() => expect(refreshPreemptionQuietly).toHaveBeenCalled());
  });

  it('a failed or stale check can be re-run; a running one cannot be doubled', () => {
    setFeed(40, { state: 'failed', ecosystems: 2, synced_at: null, error: 'npm: timeout', in_flight: false });
    const { unmount } = render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-advisories-check-now')).not.toBeDisabled();
    unmount();
    setFeed(40, { ...READY, state: 'stale', in_flight: true });
    render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-advisories-check-now')).toBeDisabled();
  });

  it('every project skipped as a temp copy: names that reason, not coverage', () => {
    setFeed(12, { state: 'no_inventory', ecosystems: 0, synced_at: null, error: null, skipped_temp: 2 });
    render(<PreemptionWorklist />);
    const notice = screen.getByTestId('preemption-advisories-uncovered');
    expect(notice).toHaveTextContent('preemption.advisories.tempSkipped.title');
    expect(notice).not.toHaveTextContent('preemption.advisories.uncovered.title');
  });

  it('synced with zero findings: the all-clear is allowed', () => {
    setFeed(40, READY);
    render(<PreemptionWorklist />);
    expect(screen.getByText(CLEAN)).toBeInTheDocument();
    expect(screen.queryByTestId(/preemption-advisories-/)).toBeNull();
  });

  it('a small project far below 100 fetched items: clean once synced', () => {
    // Three stack sources with a handful of items: the retired gate's sum is 7.
    const sourceHealth = [
      { source_type: 'crates_io', status: 'healthy', last_success_relative: '1h', items_fetched: 3, gap_message: null },
      { source_type: 'osv', status: 'healthy', last_success_relative: '1h', items_fetched: 2, gap_message: null },
      { source_type: 'cve', status: 'healthy', last_success_relative: '1h', items_fetched: 2, gap_message: null },
    ];
    setFeed(5, READY, { sourceHealth });
    render(<PreemptionWorklist />);
    expect(screen.getByText(CLEAN)).toBeInTheDocument();
  });

  it('interests on or off make no difference to the claim', () => {
    const stackOnly = [
      { source_type: 'npm_registry', status: 'healthy', last_success_relative: '1h', items_fetched: 4, gap_message: null },
    ];
    const withInterests = [
      ...stackOnly,
      { source_type: 'hackernews', status: 'healthy', last_success_relative: '1h', items_fetched: 400, gap_message: null },
      { source_type: 'reddit', status: 'healthy', last_success_relative: '1h', items_fetched: 300, gap_message: null },
    ];
    for (const sync of [READY, { state: 'syncing', ecosystems: 1, synced_at: null, error: null }]) {
      const rendered = [stackOnly, withInterests].map((sourceHealth) => {
        setFeed(20, sync, { sourceHealth });
        const { container, unmount } = render(<PreemptionWorklist />);
        const text = container.textContent;
        unmount();
        return text;
      });
      expect(rendered[0]).toBe(rendered[1]);
    }
  });

  it('a backend without the readiness field stays silent, never a false all-clear', () => {
    setFeed(40, null);
    const { container } = render(<PreemptionWorklist />);
    expect(screen.queryByText(CLEAN)).toBeNull();
    expect(container.textContent).toBe('');
  });

  it('real findings render whatever the advisory state', () => {
    setFeed(40, { state: 'syncing', ecosystems: 2, synced_at: null, error: null });
    const feed = mockState.preemptionFeed as Record<string, unknown>;
    mockState.preemptionFeed = { ...feed, total: 1, high_count: 1, items: [finding()] };
    render(<PreemptionWorklist />);
    expect(screen.getByText('preemption.alert')).toBeInTheDocument();
    expect(screen.queryByTestId(/preemption-advisories-/)).toBeNull();
  });
});

function finding() {
  return {
    id: 'osv-1', kind: 'alert', title: 'lodash prototype pollution', explanation: 'x',
    confidence: { value: 0.9, provenance: 'osv_verified', sample_size: null },
    urgency: 'high', reversibility: null, evidence: [], evidence_total: 0,
    affected_projects: [], affected_deps: ['lodash'], suggested_actions: [],
    precedents: [], refutation_condition: null,
    lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: false, no_coverage: false },
    created_at: 0, expires_at: null,
  };
}

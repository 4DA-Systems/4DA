// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { PreemptionWorklist } from './PreemptionWorklist';
import { VISIBLE_REFRESH_MIN_GAP_MS } from './PreemptionAdvisoryNotice';

// 2026-11-11: with the network blocked the advisory mirror kept reading as
// freshly synced, and the worklist had nowhere to say otherwise once it had
// items: the failed/stale notice showed only for an EMPTY list, the tab
// loaded once on mount, and there was no refresh and no "as of".

vi.mock('../SignalUpgradeCTA', () => ({ SignalUpgradeCTA: () => <div /> }));

// The key with its values, so a test can see what a string was given.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => (opts ? `${key} ${JSON.stringify(opts)}` : key),
    i18n: { language: 'en' },
  }),
}));

const cmdMock = vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve({}));
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

type Sync = {
  state: string; ecosystems: number; synced_at: string | null; error: string | null; in_flight?: boolean;
};
const READY: Sync = { state: 'ready', ecosystems: 2, synced_at: '2026-11-09 04:00:00', error: null };
const FAILED: Sync = {
  state: 'failed', ecosystems: 2, synced_at: '2026-11-09 04:00:00',
  error: 'npm: could not check for newer advisories (HEAD request failed)',
};

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

function setFeed(advisorySync: Sync | null, withFinding = true, extra: Record<string, unknown> = {}) {
  const items = withFinding ? [finding()] : [];
  mockState = {
    sourceHealth: [],
    preemptionFeed: {
      items, total: items.length, critical_count: 0, high_count: items.length, score: null,
      total_tracked: 40, weak_match_count: null, data_freshness: null, tier_scope: 'full',
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
    runAutoDiscovery: vi.fn(() => Promise.resolve()),
    loadUserContext: vi.fn(() => Promise.resolve()),
    isScanning: false,
    setShowSettings: vi.fn(),
    setSettingsInitialTab: vi.fn(),
    ...extra,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  cmdMock.mockImplementation(() => Promise.resolve({}));
});

describe('PreemptionWorklist — a failed or stale advisory check shows above a list WITH items', () => {
  it('a failed check: the line says so, with the age of the data, beside the findings', () => {
    setFeed(FAILED);
    render(<PreemptionWorklist />);
    const line = screen.getByTestId('preemption-advisories-line');
    expect(line).toHaveAttribute('data-kind', 'failed');
    expect(line).toHaveTextContent('preemption.advisories.line.failed');
    expect(line).toHaveTextContent('"age":');
    // The findings still render under it.
    expect(screen.getByText(/preemption\.header\.onList/)).toBeInTheDocument();
    expect(screen.getByTestId('preemption-advisories-check-now')).not.toBeDisabled();
  });

  it('a failed check with no verified data at all claims no age', () => {
    setFeed({ ...FAILED, synced_at: null });
    render(<PreemptionWorklist />);
    const line = screen.getByTestId('preemption-advisories-line');
    expect(line).toHaveTextContent('preemption.advisories.line.failedUndated');
    expect(line).not.toHaveTextContent('"age"');
  });

  it('out-of-date data: the line says when it was last verified', () => {
    setFeed({ ...READY, state: 'stale', synced_at: '2026-11-01 06:00:00' });
    render(<PreemptionWorklist />);
    const line = screen.getByTestId('preemption-advisories-line');
    expect(line).toHaveAttribute('data-kind', 'stale');
    expect(line).toHaveTextContent('preemption.advisories.line.stale');
  });

  it('"Check now" on the line runs the check and asks again', async () => {
    setFeed(FAILED);
    render(<PreemptionWorklist />);
    fireEvent.click(screen.getByTestId('preemption-advisories-check-now'));
    expect(cmdMock).toHaveBeenCalledWith('osv_sync_now');
    await waitFor(() => expect(refreshPreemptionQuietly).toHaveBeenCalled());
  });

  it('a retry already running cannot be doubled', () => {
    setFeed({ ...FAILED, in_flight: true });
    render(<PreemptionWorklist />);
    expect(screen.getByTestId('preemption-advisories-check-now')).toBeDisabled();
  });

  it('current data: no line, and the list says when the advisories were checked', () => {
    setFeed(READY);
    render(<PreemptionWorklist />);
    expect(screen.queryByTestId('preemption-advisories-line')).toBeNull();
    expect(screen.getByTestId('preemption-checked-at')).toHaveTextContent('preemption.advisories.asOf');
  });

  it('a check that is running has its own notice only when the list is empty', () => {
    setFeed({ state: 'syncing', ecosystems: 2, synced_at: null, error: null });
    render(<PreemptionWorklist />);
    expect(screen.queryByTestId('preemption-advisories-line')).toBeNull();
    expect(screen.queryByTestId('preemption-checked-at')).toBeNull();
  });

  it('an empty list with a failed check gives the age of the data in the notice', () => {
    setFeed(FAILED, false);
    render(<PreemptionWorklist />);
    const notice = screen.getByTestId('preemption-advisories-failed');
    expect(notice).toHaveTextContent('preemption.advisories.failed.bodyDated');
    expect(notice).toHaveTextContent('npm: could not check for newer advisories');
    expect(screen.queryByTestId('preemption-advisories-line')).toBeNull();
    expect(screen.queryByText('preemption.empty.title')).toBeNull();
  });
});

describe('PreemptionWorklist — the backend error object is rendered as text', () => {
  // The backend rejects with `{code, title, detail, ...}`; `String(e)` put
  // "[object Object]" in the notice.
  it('a failed "Check now" names the reason', async () => {
    setFeed(FAILED);
    cmdMock.mockImplementation(() => Promise.reject({
      code: 'internal', title: 'Advisory check', detail: 'The gate is closed for maintenance',
      remediation: [], severity: 'error',
    }));
    render(<PreemptionWorklist />);
    fireEvent.click(screen.getByTestId('preemption-advisories-check-now'));
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('preemption.advisories.checkFailed');
    expect(alert).toHaveTextContent('The gate is closed for maintenance');
    expect(alert).not.toHaveTextContent('[object Object]');
  });
});

describe('PreemptionWorklist — refresh', () => {
  afterEach(() => {
    vi.useRealTimers();
    Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
  });

  it('has a Refresh control that reloads the list', () => {
    setFeed(READY);
    render(<PreemptionWorklist />);
    loadPreemption.mockClear();
    const refresh = screen.getByTestId('preemption-refresh');
    expect(refresh).toHaveTextContent('preemption.refresh');
    fireEvent.click(refresh);
    expect(loadPreemption).toHaveBeenCalledTimes(1);
  });

  it('shows the refresh as running while the list loads', () => {
    setFeed(READY, true, { preemptionLoading: true });
    render(<PreemptionWorklist />);
    const refresh = screen.getByTestId('preemption-refresh');
    expect(refresh).toBeDisabled();
    expect(refresh).toHaveTextContent('preemption.refreshing');
  });

  it('asks again when the window is focused or shown again, not on every flicker', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-11-11T02:00:00Z'));
    setFeed(READY);
    render(<PreemptionWorklist />);
    expect(refreshPreemptionQuietly).not.toHaveBeenCalled();

    // Focused again right after the load: nothing new to ask.
    window.dispatchEvent(new Event('focus'));
    expect(refreshPreemptionQuietly).not.toHaveBeenCalled();

    vi.setSystemTime(Date.now() + VISIBLE_REFRESH_MIN_GAP_MS + 1);
    window.dispatchEvent(new Event('focus'));
    expect(refreshPreemptionQuietly).toHaveBeenCalledTimes(1);
    window.dispatchEvent(new Event('focus'));
    document.dispatchEvent(new Event('visibilitychange'));
    expect(refreshPreemptionQuietly).toHaveBeenCalledTimes(1);

    // Back from the tray hours later.
    vi.setSystemTime(Date.now() + 3 * 60 * 60 * 1000);
    document.dispatchEvent(new Event('visibilitychange'));
    expect(refreshPreemptionQuietly).toHaveBeenCalledTimes(2);
  });

  it('does not ask while the window is hidden', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-11-11T02:00:00Z'));
    setFeed(READY);
    render(<PreemptionWorklist />);
    vi.setSystemTime(Date.now() + VISIBLE_REFRESH_MIN_GAP_MS + 1);
    Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true });
    document.dispatchEvent(new Event('visibilitychange'));
    expect(refreshPreemptionQuietly).not.toHaveBeenCalled();
  });

  it('stops listening when the tab is left', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-11-11T02:00:00Z'));
    setFeed(READY);
    const { unmount } = render(<PreemptionWorklist />);
    unmount();
    vi.setSystemTime(Date.now() + VISIBLE_REFRESH_MIN_GAP_MS + 1);
    window.dispatchEvent(new Event('focus'));
    expect(refreshPreemptionQuietly).not.toHaveBeenCalled();
  });
});

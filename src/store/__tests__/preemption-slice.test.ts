// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { cmd } from '../../lib/commands';

vi.mock('../../lib/commands', () => ({ cmd: vi.fn() }));
const mockCmd = vi.mocked(cmd);

const initialState = useAppStore.getState();

const GATE_ERROR = 'Preemption Radar requires 4DA Signal — start your free trial or upgrade to unlock it.';

// AB-011 (display-contradicts-data): the Preemption Signal gate must render as
// an upgrade CTA, not a red error banner. This is the slice that shipped the fix
// (c5f058a5) and was later refactored onto the shared isSignalGateError helper
// (dca94dc2) — these tests pin both the branch and that the shared helper still
// classifies the gate correctly after centralization.
describe('preemption-slice — paywall classification', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    mockCmd.mockReset();
  });

  it('initial state: not paywalled, no error', () => {
    const s = useAppStore.getState();
    expect(s.preemptionPaywalled).toBe(false);
    expect(s.preemptionError).toBeNull();
  });

  it('routes a Signal-gate rejection to paywalled, NOT error', async () => {
    mockCmd.mockRejectedValue(GATE_ERROR);
    await useAppStore.getState().loadPreemption();
    const s = useAppStore.getState();
    expect(s.preemptionPaywalled).toBe(true);
    expect(s.preemptionError).toBeNull();
    expect(s.preemptionLoading).toBe(false);
  });

  it('routes a genuine fault to error, NOT paywalled', async () => {
    mockCmd.mockRejectedValue('Request timed out');
    await useAppStore.getState().loadPreemption();
    const s = useAppStore.getState();
    expect(s.preemptionPaywalled).toBe(false);
    expect(s.preemptionError).toBeTruthy();
    expect(s.preemptionLoading).toBe(false);
  });

  it('clears the paywall flag on a subsequent successful load', async () => {
    mockCmd.mockRejectedValue(GATE_ERROR);
    await useAppStore.getState().loadPreemption();
    expect(useAppStore.getState().preemptionPaywalled).toBe(true);

    mockCmd.mockResolvedValue({ items: [], summary: {} });
    await useAppStore.getState().loadPreemption();
    const s = useAppStore.getState();
    expect(s.preemptionPaywalled).toBe(false);
    expect(s.preemptionFeed).toBeTruthy();
  });
});

// AD-035: the backend owns the visibility filter — the slice's job is to hand
// it the persisted local dismissals (and the plan-expansion scope) and store
// whatever comes back, verbatim. Dismiss/undo persist locally then refetch so
// items and counts always move in the same response.
describe('preemption-slice — backend-owned visibility (AD-035)', () => {
  const DISMISS_KEY = 'preemption_dismissed';

  beforeEach(() => {
    useAppStore.setState(initialState, true);
    mockCmd.mockReset();
    mockCmd.mockResolvedValue({ items: [], total: 0, critical_count: 0, high_count: 0 });
    localStorage.removeItem(DISMISS_KEY);
  });

  it('sends the persisted dismissal ids and the plan scope with every load', async () => {
    localStorage.setItem(
      DISMISS_KEY,
      JSON.stringify([
        { id: 'osv-lodash', ts: Date.now() },
        { id: 'llm-42', ts: Date.now() },
      ]),
    );
    await useAppStore.getState().loadPreemption();
    expect(mockCmd).toHaveBeenCalledWith('get_preemption_alerts', {
      dismissedIds: ['llm-42', 'osv-lodash'],
      fullPlan: false,
    });
  });

  it('expired dismissals (7-day TTL) are pruned before the call', async () => {
    const eightDaysAgo = Date.now() - 8 * 24 * 60 * 60 * 1000;
    localStorage.setItem(
      DISMISS_KEY,
      JSON.stringify([
        { id: 'stale-alert', ts: eightDaysAgo },
        { id: 'fresh-alert', ts: Date.now() },
      ]),
    );
    await useAppStore.getState().loadPreemption();
    expect(mockCmd).toHaveBeenCalledWith('get_preemption_alerts', {
      dismissedIds: ['fresh-alert'],
      fullPlan: false,
    });
  });

  it('dismissPreemptionItem persists the id, arms undo, and refetches with it', async () => {
    await useAppStore.getState().dismissPreemptionItem('osv-axios');
    expect(useAppStore.getState().preemptionLastDismissed).toBe('osv-axios');
    expect(mockCmd).toHaveBeenLastCalledWith('get_preemption_alerts', {
      dismissedIds: ['osv-axios'],
      fullPlan: false,
    });
  });

  it('undoPreemptionDismissal removes the id and refetches without it', async () => {
    await useAppStore.getState().dismissPreemptionItem('osv-axios');
    await useAppStore.getState().undoPreemptionDismissal();
    expect(useAppStore.getState().preemptionLastDismissed).toBeNull();
    expect(mockCmd).toHaveBeenLastCalledWith('get_preemption_alerts', {
      dismissedIds: [],
      fullPlan: false,
    });
  });

  it('expandPreemptionPlan refetches with fullPlan and keeps it for later loads', async () => {
    await useAppStore.getState().expandPreemptionPlan();
    expect(mockCmd).toHaveBeenLastCalledWith('get_preemption_alerts', {
      dismissedIds: [],
      fullPlan: true,
    });
    // A dismissal after expansion must not collapse the plan again.
    await useAppStore.getState().dismissPreemptionItem('osv-axios');
    expect(mockCmd).toHaveBeenLastCalledWith('get_preemption_alerts', {
      dismissedIds: ['osv-axios'],
      fullPlan: true,
    });
  });
});

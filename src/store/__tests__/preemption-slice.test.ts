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

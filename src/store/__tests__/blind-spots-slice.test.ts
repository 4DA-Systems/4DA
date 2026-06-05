// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { cmd } from '../../lib/commands';

vi.mock('../../lib/commands', () => ({ cmd: vi.fn() }));
const mockCmd = vi.mocked(cmd);

const initialState = useAppStore.getState();

const GATE_ERROR = 'Blind Spots requires 4DA Signal — start your free trial or upgrade to unlock it.';

// AB-011 (display-contradicts-data): a Signal-tier paywall must surface as an
// upgrade path, never a red error banner. The slice is where the gate rejection
// is classified — these pin that branch so a free-tier user can never again get
// "Something went wrong" on the Blind Spots tab.
describe('blind-spots-slice — paywall classification', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    mockCmd.mockReset();
  });

  it('initial state: not paywalled, no error', () => {
    const s = useAppStore.getState();
    expect(s.blindSpotsPaywalled).toBe(false);
    expect(s.blindSpotsError).toBeNull();
  });

  it('routes a Signal-gate rejection to paywalled, NOT error', async () => {
    mockCmd.mockRejectedValue(GATE_ERROR);
    await useAppStore.getState().loadBlindSpots();
    const s = useAppStore.getState();
    expect(s.blindSpotsPaywalled).toBe(true);
    expect(s.blindSpotsError).toBeNull();
    expect(s.blindSpotsLoading).toBe(false);
  });

  it('routes a genuine fault to error, NOT paywalled', async () => {
    mockCmd.mockRejectedValue('database is locked');
    await useAppStore.getState().loadBlindSpots();
    const s = useAppStore.getState();
    expect(s.blindSpotsPaywalled).toBe(false);
    expect(s.blindSpotsError).toBeTruthy();
    expect(s.blindSpotsLoading).toBe(false);
  });

  it('clears the paywall flag on a subsequent successful load', async () => {
    mockCmd.mockRejectedValue(GATE_ERROR);
    await useAppStore.getState().loadBlindSpots();
    expect(useAppStore.getState().blindSpotsPaywalled).toBe(true);

    mockCmd.mockResolvedValue({ items: [], score: 0 });
    await useAppStore.getState().loadBlindSpots();
    const s = useAppStore.getState();
    expect(s.blindSpotsPaywalled).toBe(false);
    expect(s.blindSpotReport).toBeTruthy();
  });
});

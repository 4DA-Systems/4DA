// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

let emit: ((pulse: number) => void) | null = null;
const initial = vi.hoisted((): { pulse: number | null } => ({ pulse: null }));

vi.mock('../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve(initial.pulse === null ? null : { pulse: initial.pulse })),
}));
vi.mock('../lib/tauri-events', () => ({
  safeListen: vi.fn((_event: string, handler: (e: { payload: { pulse: number } }) => void) => {
    emit = (pulse) => handler({ payload: { pulse } });
    return Promise.resolve(() => {});
  }),
}));

import { useBackgroundWork, isWorkEvidence, WORK_FRESHNESS_MS } from './use-background-work';

/** Flush the mount-time get_void_signal promise under fake timers. */
async function mount() {
  const hook = renderHook(() => useBackgroundWork());
  await act(async () => {
    await Promise.resolve();
  });
  return hook;
}

describe('useBackgroundWork — pulse counts only while fresh', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    emit = null;
    initial.pulse = null;
  });
  afterEach(() => vi.useRealTimers());

  it('pulse 0.8 with no further updates drops to rest after the window; a new update re-activates', async () => {
    const { result } = await mount();
    expect(result.current).toBe(false);

    act(() => emit!(0.8));
    expect(result.current).toBe(true);

    // The missed final pulse=0: nothing more arrives. The timer alone ends it.
    act(() => vi.advanceTimersByTime(WORK_FRESHNESS_MS - 1));
    expect(result.current).toBe(true);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toBe(false);

    // A new fetch starts.
    act(() => emit!(1.0));
    expect(result.current).toBe(true);
  });

  it('the minute staleness tick (a decayed pulse) never re-freshens a stale fetch', async () => {
    const { result } = await mount();
    act(() => emit!(1.0));
    act(() => vi.advanceTimersByTime(WORK_FRESHNESS_MS));
    expect(result.current).toBe(false);
    // tick_staleness re-emits prev * 0.98 every 60 s.
    for (let pulse = 0.98; pulse >= 0.35; pulse *= 0.98) {
      act(() => emit!(pulse));
      act(() => vi.advanceTimersByTime(60_000));
      expect(result.current).toBe(false);
    }
  });

  it('per-source progress keeps it alive; wind-down and cycle end drop it at once', async () => {
    const { result } = await mount();
    act(() => emit!(1.0)); // fetch start
    for (const p of [0.5, 0.6, 0.8, 1.0]) {
      act(() => vi.advanceTimersByTime(WORK_FRESHNESS_MS - 1000));
      act(() => emit!(p));
      expect(result.current).toBe(true);
    }
    act(() => emit!(0.3)); // cache filled, winding down
    expect(result.current).toBe(false);
    act(() => emit!(0)); // analysis landed
    expect(result.current).toBe(false);
  });

  it('a mid-fetch snapshot at mount counts, and still expires', async () => {
    initial.pulse = 0.9;
    const { result } = await mount();
    expect(result.current).toBe(true);
    act(() => vi.advanceTimersByTime(WORK_FRESHNESS_MS));
    expect(result.current).toBe(false);
  });

  it('isWorkEvidence: working-level and not falling', () => {
    expect(isWorkEvidence(1, 0)).toBe(true);
    expect(isWorkEvidence(0.6, 0.5)).toBe(true);
    expect(isWorkEvidence(1, 1)).toBe(true);
    expect(isWorkEvidence(0.98, 1)).toBe(false);
    expect(isWorkEvidence(0.3, 0)).toBe(false);
  });
});

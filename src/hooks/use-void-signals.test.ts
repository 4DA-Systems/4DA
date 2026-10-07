// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import type { VoidSignal } from '../types';
import { installRafHarness, type RafHarness } from '../test/motion-test-utils';

let emit: ((payload: VoidSignal) => void) | null = null;

vi.mock('../lib/commands', () => ({ cmd: vi.fn(() => Promise.resolve(null)) }));
vi.mock('../lib/tauri-events', () => ({
  safeListen: vi.fn((_event: string, handler: (e: { payload: VoidSignal }) => void) => {
    emit = (payload) => handler({ payload });
    return Promise.resolve(() => {});
  }),
}));

import {
  useVoidSignals,
  stepVoidSignal,
  voidSignalConverged,
  normalizeVoidSignal,
} from './use-void-signals';

const TARGET: VoidSignal = normalizeVoidSignal({
  pulse: 0.8,
  heat: 0.6,
  staleness: 0.1,
  item_count: 42,
  signal_intensity: 0.5,
});

describe('useVoidSignals loop lifecycle', () => {
  let raf: RafHarness;

  beforeEach(() => {
    emit = null;
    raf = installRafHarness();
  });
  afterEach(() => raf.restore());

  const mountAndListen = async () => {
    const hook = renderHook(() => useVoidSignals());
    await waitFor(() => expect(emit).not.toBeNull());
    return hook;
  };

  it('schedules no frame while current already equals target (idle)', async () => {
    await mountAndListen();
    expect(raf.pending()).toBe(0);
  });

  it('runs on a new target, then stops once converged', async () => {
    const { result } = await mountAndListen();
    act(() => emit!(TARGET));
    expect(raf.pending()).toBe(1);

    let time = 0;
    let frames = 0;
    while (raf.pending() > 0 && frames < 5000) {
      time += 40;
      act(() => raf.flush(time));
      frames += 1;
    }
    expect(raf.pending()).toBe(0); // loop stopped by itself
    expect(frames).toBeLessThan(5000);
    expect(voidSignalConverged(result.current, TARGET)).toBe(true);

    // A further event with the same target does not restart the loop.
    act(() => emit!(TARGET));
    expect(raf.pending()).toBe(0);
  });

  it('schedules no frame while the document is hidden, resumes when shown', async () => {
    await mountAndListen();
    act(() => raf.setHidden(true));
    act(() => emit!(TARGET));
    expect(raf.pending()).toBe(0);

    act(() => raf.setHidden(false));
    expect(raf.pending()).toBe(1);
  });

  it('cancels a running loop when the document becomes hidden', async () => {
    await mountAndListen();
    act(() => emit!(TARGET));
    expect(raf.pending()).toBe(1);
    act(() => raf.setHidden(true));
    expect(raf.pending()).toBe(0);
  });

  it('applies the target directly with no frames under reduced motion', async () => {
    raf.setReducedMotion(true);
    const { result } = await mountAndListen();
    act(() => emit!(TARGET));
    expect(raf.pending()).toBe(0);
    expect(result.current).toEqual(TARGET);
  });

  it('cancels its frame on unmount', async () => {
    const { unmount } = await mountAndListen();
    act(() => emit!(TARGET));
    expect(raf.pending()).toBe(1);
    unmount();
    expect(raf.pending()).toBe(0);
  });
});

describe('void-signal interpolation helpers', () => {
  it('stepping repeatedly reaches the target exactly', () => {
    let cur = normalizeVoidSignal({});
    for (let i = 0; i < 2000 && !voidSignalConverged(cur, TARGET); i++) {
      cur = stepVoidSignal(cur, TARGET);
    }
    expect(voidSignalConverged(cur, TARGET)).toBe(true);
  });

  it('fills missing and non-finite fields so the loop can converge', () => {
    const s = normalizeVoidSignal({ pulse: Number.NaN, heat: 0.4 });
    expect(s.pulse).toBe(0);
    expect(s.heat).toBe(0.4);
    expect(s.advantage_trend).toBe(0);
  });
});

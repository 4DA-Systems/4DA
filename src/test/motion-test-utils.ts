// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Test harness for animation loops: a manual requestAnimationFrame queue plus
 * switches for document visibility, window focus and prefers-reduced-motion.
 */
import { vi } from 'vitest';

export interface RafHarness {
  /** rAF callbacks currently scheduled (not yet run or cancelled). */
  pending: () => number;
  /** Run every currently scheduled callback at `time`. */
  flush: (time: number) => void;
  setHidden: (hidden: boolean) => void;
  /** Window focus (`document.hasFocus()`), announced with a focus/blur event. Starts focused. */
  setFocused: (focused: boolean) => void;
  setReducedMotion: (reduce: boolean) => void;
  restore: () => void;
}

export function installRafHarness(): RafHarness {
  let nextId = 1;
  const queue = new Map<number, FrameRequestCallback>();
  let hidden = false;
  let focused = true;
  let reduce = false;
  const mqlListeners = new Set<() => void>();

  const rafSpy = vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb) => {
    const id = nextId++;
    queue.set(id, cb);
    return id;
  });
  const cafSpy = vi.spyOn(window, 'cancelAnimationFrame').mockImplementation((id) => {
    queue.delete(id);
  });
  const hiddenSpy = vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden);
  const focusSpy = vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
  const originalMatchMedia = window.matchMedia;
  window.matchMedia = ((query: string) => ({
    matches: query.includes('prefers-reduced-motion') ? reduce : false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: (_: string, fn: () => void) => mqlListeners.add(fn),
    removeEventListener: (_: string, fn: () => void) => mqlListeners.delete(fn),
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;

  return {
    pending: () => queue.size,
    flush: (time) => {
      const batch = [...queue.entries()];
      queue.clear();
      for (const [, cb] of batch) cb(time);
    },
    setHidden: (h) => {
      hidden = h;
      document.dispatchEvent(new Event('visibilitychange'));
    },
    setFocused: (f) => {
      focused = f;
      window.dispatchEvent(new Event(f ? 'focus' : 'blur'));
    },
    setReducedMotion: (r) => {
      reduce = r;
      for (const fn of [...mqlListeners]) fn();
    },
    restore: () => {
      rafSpy.mockRestore();
      cafSpy.mockRestore();
      hiddenSpy.mockRestore();
      focusSpy.mockRestore();
      window.matchMedia = originalMatchMedia;
    },
  };
}

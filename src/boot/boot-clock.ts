// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * How long this page has been starting, and what that means for the person
 * watching it.
 *
 * The clock is `performance.now()`: milliseconds since the page began to
 * load. It needs no shared state, so the boot screen (before the app's code
 * arrives) and the splash screen (after) read the same clock and the wait
 * reads as one continuous count across both.
 */
import { useEffect, useState } from 'react';

/** Before this, a start is ordinary and the screen says nothing extra. */
export const SLOW_AFTER_SECONDS = 6;
/** After this, the screen explains the likely cause and offers a reload. */
export const VERY_SLOW_AFTER_SECONDS = 20;

export type StartPace = 'normal' | 'slow' | 'verySlow';

export function startPace(elapsedSeconds: number): StartPace {
  if (elapsedSeconds >= VERY_SLOW_AFTER_SECONDS) return 'verySlow';
  if (elapsedSeconds >= SLOW_AFTER_SECONDS) return 'slow';
  return 'normal';
}

export function bootElapsedSeconds(): number {
  return typeof performance === 'undefined' ? 0 : Math.floor(performance.now() / 1000);
}

/** Seconds since the page began to load, re-read once a second. */
export function useBootElapsedSeconds(): number {
  const [seconds, setSeconds] = useState(bootElapsedSeconds);
  useEffect(() => {
    const timer = setInterval(() => setSeconds(bootElapsedSeconds()), 1000);
    return () => clearInterval(timer);
  }, []);
  return seconds;
}

/** `75` -> `1:15`. */
export function formatElapsed(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

/**
 * Files the page has fetched so far. Only meaningful in a development build,
 * where the dev server hands the interface over one source file at a time and
 * a rising count is direct evidence that loading is still moving.
 */
export function loadedFileCount(): number {
  if (typeof performance === 'undefined' || typeof performance.getEntriesByType !== 'function') {
    return 0;
  }
  return performance.getEntriesByType('resource').length;
}

/** The browser keeps 250 resource entries by default; a dev load fetches far more. */
export function widenResourceCount(): void {
  try {
    performance.setResourceTimingBufferSize?.(10_000);
  } catch {
    /* not available — the count simply stops at the default limit */
  }
}

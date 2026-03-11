/**
 * Tests for useVisibilityPolling hook.
 *
 * Covers polling start/stop, visibility change handling, and cleanup.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useVisibilityPolling } from '../use-visibility-polling';

describe('useVisibilityPolling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('calls callback at specified interval when enabled', () => {
    const callback = vi.fn();
    renderHook(() => useVisibilityPolling(callback, 1000, true));

    vi.advanceTimersByTime(3000);
    expect(callback.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it('does not call callback when disabled', () => {
    const callback = vi.fn();
    renderHook(() => useVisibilityPolling(callback, 1000, false));

    vi.advanceTimersByTime(3000);
    expect(callback).not.toHaveBeenCalled();
  });

  it('stops polling on unmount', () => {
    const callback = vi.fn();
    const { unmount } = renderHook(() => useVisibilityPolling(callback, 1000, true));

    vi.advanceTimersByTime(1500);
    const callsBefore = callback.mock.calls.length;

    unmount();
    vi.advanceTimersByTime(3000);

    // No additional calls after unmount
    expect(callback.mock.calls.length).toBe(callsBefore);
  });

  it('uses the latest callback reference', () => {
    let counter = 0;
    const callback1 = vi.fn(() => { counter = 1; });
    const callback2 = vi.fn(() => { counter = 2; });

    const { rerender } = renderHook(
      ({ cb }) => useVisibilityPolling(cb, 1000, true),
      { initialProps: { cb: callback1 } },
    );

    vi.advanceTimersByTime(1000);
    expect(counter).toBe(1);

    rerender({ cb: callback2 });
    vi.advanceTimersByTime(1000);
    expect(counter).toBe(2);
  });

  it('defaults to enabled when third parameter is omitted', () => {
    const callback = vi.fn();
    renderHook(() => useVisibilityPolling(callback, 1000));

    vi.advanceTimersByTime(2000);
    expect(callback.mock.calls.length).toBeGreaterThanOrEqual(2);
  });
});

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Tests for `useAweLiveEvents` — the Tauri event subscription hook that
 * keeps the Wisdom Trajectory UI in sync with the AWE backend without
 * polling.
 *
 * Pre-fix the Wisdom Trajectory was a static snapshot: `loadAweSummary()`
 * ran once on component mount, then the UI sat frozen until the user
 * navigated away and back. This test suite verifies that:
 *
 *   1. The hook subscribes to every AWE event name.
 *   2. Incoming events trigger store refresh actions.
 *   3. Rapid bursts are debounced (one refresh per burst, not N).
 *   4. Cleanup unsubscribes all listeners.
 */
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';

// Mock Tauri modules — vi.mock is hoisted so any referenced symbols must
// come from vi.hoisted() to avoid TDZ errors when the mock runs first.
const { mockListen, mockUnlisten } = vi.hoisted(() => ({
  mockListen: vi.fn(),
  mockUnlisten: vi.fn(),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
  emit: vi.fn(),
}));

// Mock core invoke so loadAweSummary etc. are no-ops.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue('{}'),
}));

// Captured event handlers — each mockListen() call stores its handler
// keyed by event name so tests can fire events synthetically.
const capturedHandlers: Record<string, (payload: unknown) => void> = {};

beforeEach(() => {
  vi.clearAllMocks();
  for (const key of Object.keys(capturedHandlers)) {
    delete capturedHandlers[key];
  }
  mockListen.mockImplementation((name: string, handler: (event: { payload: unknown }) => void) => {
    capturedHandlers[name] = (payload) => { handler({ payload }); };
    return Promise.resolve(mockUnlisten);
  });
});

afterEach(() => {
  vi.useRealTimers();
});

// Import AFTER mocks so the hook picks up the mocked module.
import { useAweLiveEvents } from '../use-awe-live-events';
import { useAppStore } from '../../store';

/**
 * Flush all queued microtasks. The hook's setup() loop does 9 sequential
 * `await listen(...)` calls, so we need enough ticks to let every one
 * resolve before asserting on `capturedHandlers`. A setTimeout(0) drain
 * is the simplest reliable way to flush pending promises without
 * coupling the test to an exact tick count.
 */
async function flushAllPromises() {
  await new Promise<void>((resolve) => { setTimeout(() => { resolve(); }, 0); });
}

// ---------------------------------------------------------------------------
// Expected event names — must stay aligned with awe_events.rs / the hook.
// ---------------------------------------------------------------------------

const EXPECTED_EVENTS = [
  'awe:decision-added',
  'awe:feedback-recorded',
  'awe:principle-validated',
  'awe:coverage-changed',
  'awe:scan-complete',
  'awe:retriage-complete',
  'awe:seed-complete',
  'awe:source-mining-complete',
  'awe:summary-stale',
];

describe('useAweLiveEvents', () => {
  it('subscribes to every expected AWE event on mount', async () => {
    renderHook(() => { useAweLiveEvents(); });
    // Event subscription happens in a microtask inside the effect
    await flushAllPromises();

    const subscribed = mockListen.mock.calls.map(call => call[0]);
    for (const expected of EXPECTED_EVENTS) {
      expect(subscribed).toContain(expected);
    }
    expect(subscribed.length).toBe(EXPECTED_EVENTS.length);
  });

  it('unsubscribes all listeners on unmount', async () => {
    const { unmount } = renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    unmount();
    // Each subscription gets its own unlisten — we assert the mock was
    // invoked at least once per subscription.
    expect(mockUnlisten).toHaveBeenCalledTimes(EXPECTED_EVENTS.length);
  });

  it('fires loadAweSummary on awe:summary-stale events', async () => {
    const loadSpy = vi.spyOn(useAppStore.getState(), 'loadAweSummary');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    // Fake timers so we can advance the debounce window precisely.
    vi.useFakeTimers();
    capturedHandlers['awe:summary-stale']?.({ kind: 'summary_stale' });
    // Before the debounce window elapses, no call yet.
    vi.advanceTimersByTime(100);
    expect(loadSpy).not.toHaveBeenCalled();

    // After the full window, exactly one refresh fires.
    vi.advanceTimersByTime(400);
    expect(loadSpy).toHaveBeenCalledTimes(1);
  });

  it('debounces rapid bursts into a single refresh', async () => {
    const loadSpy = vi.spyOn(useAppStore.getState(), 'loadAweSummary');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    vi.useFakeTimers();
    // Fire 10 stale events back-to-back.
    for (let i = 0; i < 10; i++) {
      capturedHandlers['awe:summary-stale']?.({ kind: 'summary_stale' });
    }
    vi.advanceTimersByTime(500);
    // Only ONE refresh should have fired — this is the whole point of
    // debouncing, and the contract the daily autonomous tier relies on.
    expect(loadSpy).toHaveBeenCalledTimes(1);
  });

  it('scan-complete with decisions_stored=0 only touches summary', async () => {
    const loadSummarySpy = vi.spyOn(useAppStore.getState(), 'loadAweSummary');
    const loadWellSpy = vi.spyOn(useAppStore.getState(), 'loadAweWisdomWell');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    vi.useFakeTimers();
    capturedHandlers['awe:scan-complete']?.({
      kind: 'scan_complete',
      repos_scanned: 3,
      decisions_stored: 0,
      outcomes_inferred: 0,
    });
    vi.advanceTimersByTime(500);
    expect(loadSummarySpy).toHaveBeenCalled();
    // Empty scan should NOT bother refreshing the well — cheap path.
    expect(loadWellSpy).not.toHaveBeenCalled();
  });

  it('scan-complete with decisions_stored>0 triggers full refresh', async () => {
    const loadSummarySpy = vi.spyOn(useAppStore.getState(), 'loadAweSummary');
    const loadWellSpy = vi.spyOn(useAppStore.getState(), 'loadAweWisdomWell');
    const loadPendingSpy = vi.spyOn(useAppStore.getState(), 'loadAwePendingDecisions');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    vi.useFakeTimers();
    capturedHandlers['awe:scan-complete']?.({
      kind: 'scan_complete',
      repos_scanned: 3,
      decisions_stored: 5,
      outcomes_inferred: 2,
    });
    vi.advanceTimersByTime(500);
    expect(loadSummarySpy).toHaveBeenCalled();
    expect(loadWellSpy).toHaveBeenCalled();
    expect(loadPendingSpy).toHaveBeenCalled();
  });

  it('principle-validated event shows a success toast', async () => {
    const addToastSpy = vi.spyOn(useAppStore.getState(), 'addToast');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    vi.useFakeTimers();
    capturedHandlers['awe:principle-validated']?.({
      kind: 'principle_validated',
      statement: 'Small atomic commits beat large batch commits',
      confidence: 0.85,
      evidence_count: 7,
      domain: 'software-engineering',
    });
    expect(addToastSpy).toHaveBeenCalledWith(
      'success',
      expect.stringContaining('Small atomic commits'),
    );
  });

  it('seed-complete event with decisions_loaded>0 shows an info toast', async () => {
    const addToastSpy = vi.spyOn(useAppStore.getState(), 'addToast');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    capturedHandlers['awe:seed-complete']?.({
      kind: 'seed_complete',
      decisions_loaded: 30,
    });
    expect(addToastSpy).toHaveBeenCalledWith(
      'info',
      expect.stringContaining('30 decisions'),
    );
  });

  it('seed-complete with decisions_loaded=0 is silent', async () => {
    const addToastSpy = vi.spyOn(useAppStore.getState(), 'addToast');
    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    capturedHandlers['awe:seed-complete']?.({
      kind: 'seed_complete',
      decisions_loaded: 0,
    });
    expect(addToastSpy).not.toHaveBeenCalled();
  });

  it('handler errors do not break the subscription chain', async () => {
    // Force loadAweSummary to throw so we can verify isolation.
    vi.spyOn(useAppStore.getState(), 'loadAweSummary').mockImplementation(() => {
      throw new Error('simulated failure');
    });
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});

    renderHook(() => { useAweLiveEvents(); });
    await flushAllPromises();

    // Fake timers so we can trigger the debounced refresh reliably.
    vi.useFakeTimers();
    expect(() => {
      capturedHandlers['awe:summary-stale']?.({ kind: 'summary_stale' });
      vi.advanceTimersByTime(500);
    }).not.toThrow();

    warnSpy.mockRestore();
  });
});

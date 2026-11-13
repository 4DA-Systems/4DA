// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useAppStore } from '../../store';
import { useBriefing } from '../use-briefing';

describe('useBriefing auto trigger', () => {
  const generateBriefing = vi.fn(() => Promise.resolve());

  beforeEach(() => {
    vi.useFakeTimers();
    generateBriefing.mockClear();
    useAppStore.setState({
      autoBriefingEnabled: true,
      generateBriefing,
      aiBriefing: { content: null, loading: false, error: null, model: null, lastGenerated: null },
      appState: { ...useAppStore.getState().appState, relevanceResults: [] },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // Audit 2026-11-11: the brief was requested only when the analysis returned
  // rows, though the facts-first brief needs none — an offline first run (zero
  // rows) never got one.
  it('asks for the brief when an analysis completes with no rows', async () => {
    const { rerender } = renderHook(({ done }) => useBriefing(done), { initialProps: { done: false } });
    rerender({ done: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(600); });
    expect(generateBriefing).toHaveBeenCalledTimes(1);
    expect(generateBriefing).toHaveBeenCalledWith({ auto: true });
  });

  it('does not ask while auto briefing is off', async () => {
    useAppStore.setState({ autoBriefingEnabled: false });
    const { rerender } = renderHook(({ done }) => useBriefing(done), { initialProps: { done: false } });
    rerender({ done: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(600); });
    expect(generateBriefing).not.toHaveBeenCalled();
  });
});

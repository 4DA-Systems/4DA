// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// PR #1131 review: the header's freshness was re-read only on mount, after an
// analysis, and after a background cycle WITH items — so a failed advisory
// check or an engine-process run left it green.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { Event } from '@tauri-apps/api/event';
import { renderHook } from '@testing-library/react';
import { useAppStore } from '../../store';
import { createBackgroundResultsHandler } from '../analysis-event-handlers';
import { MONITORING_REFRESH_MS, useMonitoring } from '../use-monitoring';
import type { SourceRelevance } from '../../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve(null)) }));

const initialState = useAppStore.getState();

describe('header freshness refresh', () => {
  const load = vi.fn(() => Promise.resolve());

  beforeEach(() => {
    useAppStore.setState(initialState, true);
    load.mockClear();
    useAppStore.setState({ loadMonitoringStatus: load });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('a background cycle that found nothing still re-reads it', () => {
    createBackgroundResultsHandler()({ payload: [] as SourceRelevance[] } as Event<SourceRelevance[]>);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it('re-reads on a timer and when the window regains focus', () => {
    vi.useFakeTimers();
    renderHook(() => useMonitoring());
    expect(load).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(MONITORING_REFRESH_MS);
    expect(load).toHaveBeenCalledTimes(2);
    window.dispatchEvent(new Event('focus'));
    expect(load).toHaveBeenCalledTimes(3);
  });
});

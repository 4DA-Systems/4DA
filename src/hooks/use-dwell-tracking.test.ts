// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useDwellTracking } from './use-dwell-tracking';

vi.mock('../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve(null)),
}));

import { cmd } from '../lib/commands';

describe('useDwellTracking', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  it('returns onVisible and onHidden callbacks', () => {
    const { result } = renderHook(() => useDwellTracking(1, 'hackernews', ['rust']));
    expect(typeof result.current.onVisible).toBe('function');
    expect(typeof result.current.onHidden).toBe('function');
  });

  it('records interaction after sufficient dwell time', () => {
    const { result } = renderHook(() => useDwellTracking(42, 'github', ['typescript']));

    act(() => result.current.onVisible());
    vi.advanceTimersByTime(10_000);
    act(() => result.current.onHidden());

    // I-1 regression guard: Tauri maps camelCase JS args -> snake_case Rust params.
    // Passing snake_case keys (item_id/action_type/...) silently fails to bind and the
    // interaction is never recorded. Keys MUST be camelCase.
    expect(cmd).toHaveBeenCalledWith('ace_record_interaction', expect.objectContaining({
      itemId: 42,
      actionType: 'click',
      itemSource: 'github',
    }));
    const args = (cmd as unknown as { mock: { calls: unknown[][] } }).mock.calls[0]![1] as Record<string, unknown>;
    expect(args).not.toHaveProperty('item_id');
    expect(args).not.toHaveProperty('action_type');
    expect(args).not.toHaveProperty('item_source');
  });

  it('ignores dwell under 2 seconds', () => {
    const { result } = renderHook(() => useDwellTracking(1, 'hn', ['go']));

    act(() => result.current.onVisible());
    vi.advanceTimersByTime(1_000);
    act(() => result.current.onHidden());

    expect(cmd).not.toHaveBeenCalled();
  });

  it('ignores dwell over 300 seconds', () => {
    const { result } = renderHook(() => useDwellTracking(1, 'hn', ['go']));

    act(() => result.current.onVisible());
    vi.advanceTimersByTime(301_000);
    act(() => result.current.onHidden());

    expect(cmd).not.toHaveBeenCalled();
  });

  it('does nothing if onHidden called without onVisible', () => {
    const { result } = renderHook(() => useDwellTracking(1, 'hn', ['go']));

    act(() => result.current.onHidden());

    expect(cmd).not.toHaveBeenCalled();
  });

  it('does nothing when itemId is null', () => {
    const { result } = renderHook(() => useDwellTracking(null, 'hn', ['go']));

    act(() => result.current.onVisible());
    vi.advanceTimersByTime(10_000);
    act(() => result.current.onHidden());

    expect(cmd).not.toHaveBeenCalled();
  });
});

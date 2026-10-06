// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ContentGraph } from '../../types/graph';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

import { clearGraphCache, useContentGraph } from './use-content-graph';

const graph = { nodes: [], edges: [], clusters: [], meta: {} } as unknown as ContentGraph;

describe('useContentGraph cache', () => {
  beforeEach(() => {
    clearGraphCache();
    cmdMock.mockReset();
    cmdMock.mockResolvedValue(graph);
  });

  it('a second view of the same window reuses the built map without a rebuild', async () => {
    const first = renderHook(() => useContentGraph(7));
    await waitFor(() => expect(first.result.current.loading).toBe(false));
    expect(cmdMock).toHaveBeenCalledTimes(1);
    first.unmount();

    const second = renderHook(() => useContentGraph(7));
    // Served synchronously from the cache: no spinner, no second build.
    expect(second.result.current.loading).toBe(false);
    expect(second.result.current.graph).toBe(graph);
    await act(async () => {});
    expect(cmdMock).toHaveBeenCalledTimes(1);
  });

  it('a different window builds its own map', async () => {
    const a = renderHook(({ d }) => useContentGraph(d), { initialProps: { d: 7 } });
    await waitFor(() => expect(a.result.current.loading).toBe(false));
    a.rerender({ d: 14 });
    await waitFor(() => expect(cmdMock).toHaveBeenCalledTimes(2));
    expect(cmdMock.mock.calls[1]![1]).toMatchObject({ days: 14 });
  });

  it('an explicit retry rebuilds even with a cached map', async () => {
    cmdMock.mockRejectedValueOnce(new Error('boom'));
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const h = renderHook(() => useContentGraph(7));
    await waitFor(() => expect(h.result.current.loadError).toBe(true));
    act(() => h.result.current.reload());
    await waitFor(() => expect(h.result.current.graph).toBe(graph));
    act(() => h.result.current.reload());
    await waitFor(() => expect(cmdMock).toHaveBeenCalledTimes(3));
    spy.mockRestore();
  });
});

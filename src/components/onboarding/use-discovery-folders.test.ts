// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));

import { useDiscoveryFolders } from './use-discovery-folders';

beforeEach(() => {
  cmdMock.mockReset();
  cmdMock.mockImplementation((command: string, params?: { path?: string }) => {
    if (command === 'ace_preview_discovery_dirs') return Promise.resolve([]);
    if (command === 'ace_check_scan_folder') {
      return Promise.resolve(params?.path === 'D:\\code' ? null : 'not_found');
    }
    return Promise.resolve(null);
  });
});

// Fresh-profile audit E9: "Add folder" accepted a path that does not exist,
// and the scan then dropped it without a word.
describe('useDiscoveryFolders — adding a folder by path', () => {
  it('refuses a folder that does not exist, with the reason', async () => {
    const { result } = renderHook(() => useDiscoveryFolders());
    await waitFor(() => expect(result.current.loading).toBe(false));
    let added = true;
    await act(async () => { added = await result.current.addFolder('D:\\nope'); });
    expect(added).toBe(false);
    expect(result.current.folders).toEqual([]);
    expect(result.current.addError).toEqual({ path: 'D:\\nope', reason: 'not_found' });
  });

  it('adds a folder that exists, ticked, and clears the error', async () => {
    const { result } = renderHook(() => useDiscoveryFolders());
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(async () => { await result.current.addFolder('D:\\nope'); });
    await act(async () => { await result.current.addFolder('  D:\\code '); });
    expect(result.current.folders).toEqual([{ path: 'D:\\code', checked: true }]);
    expect(result.current.addError).toBeNull();
  });
});

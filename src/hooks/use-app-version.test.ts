// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const getVersionMock = vi.fn();
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => getVersionMock() }));

import { useAppVersion, resetAppVersionCache } from './use-app-version';

type TauriWindow = Window & { __TAURI_INTERNALS__?: unknown };

describe('useAppVersion', () => {
  beforeEach(() => {
    resetAppVersionCache();
    getVersionMock.mockReset();
    vi.stubGlobal('__APP_VERSION__', '1.0.0');
  });
  afterEach(() => {
    delete (window as TauriWindow).__TAURI_INTERNALS__;
    vi.unstubAllGlobals();
  });

  it('reports the runtime version from Tauri, not the build fallback', async () => {
    (window as TauriWindow).__TAURI_INTERNALS__ = {};
    getVersionMock.mockResolvedValue('1.0.3');
    const { result } = renderHook(() => useAppVersion());
    await waitFor(() => expect(result.current).toBe('1.0.3'));
    // Cached: a second consumer starts with the real version and asks no more.
    const second = renderHook(() => useAppVersion());
    expect(second.result.current).toBe('1.0.3');
    expect(getVersionMock).toHaveBeenCalledTimes(1);
  });

  it('falls back to the build-time version outside Tauri', () => {
    const { result } = renderHook(() => useAppVersion());
    expect(result.current).toBe('1.0.0');
    expect(getVersionMock).not.toHaveBeenCalled();
  });

  it('keeps the build-time version when getVersion fails', async () => {
    (window as TauriWindow).__TAURI_INTERNALS__ = {};
    getVersionMock.mockRejectedValue(new Error('no ipc'));
    const { result } = renderHook(() => useAppVersion());
    await waitFor(() => expect(getVersionMock).toHaveBeenCalled());
    expect(result.current).toBe('1.0.0');
  });
});

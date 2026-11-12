// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The desktop updater (audit 2026-11-11, R5): a tray app must keep checking,
// a failed check must be visible, a large download must show progress, a
// failed install must not leave "Installing..." forever, and a finished
// install must restart into the new version.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

const checkMock = vi.fn();
const relaunchMock = vi.fn();
vi.mock('@tauri-apps/plugin-updater', () => ({ check: (...a: unknown[]) => checkMock(...a) }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: (...a: unknown[]) => relaunchMock(...a) }));
vi.mock('../lib/platform', () => ({ osFamily: () => 'windows' }));

import {
  useUpdateCheck,
  FIRST_CHECK_DELAY_MS,
  UPDATE_RECHECK_MS,
  FOCUS_RECHECK_MIN_MS,
} from './use-update-check';
import { getUpdateStatus, requestUpdateCheck, __resetUpdateStatusForTests } from './update-status';

type Ev = { event: string; data?: Record<string, number> };
function fakeUpdate(version: string, install?: (cb: (e: Ev) => void) => Promise<void>) {
  return {
    version,
    body: 'notes',
    close: vi.fn(() => Promise.resolve()),
    downloadAndInstall: vi.fn(install ?? (() => Promise.resolve())),
  };
}

const flush = () => act(async () => { await Promise.resolve(); await Promise.resolve(); });

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubEnv('DEV', false);
  __resetUpdateStatusForTests();
  checkMock.mockReset();
  relaunchMock.mockReset();
  relaunchMock.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.useRealTimers();
});

describe('useUpdateCheck scheduling', () => {
  it('checks after start and again every 6 hours, not just once per process', async () => {
    checkMock.mockResolvedValue(null);
    renderHook(() => useUpdateCheck());
    expect(checkMock).toHaveBeenCalledTimes(0);
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    expect(checkMock).toHaveBeenCalledTimes(1);
    await act(async () => { vi.advanceTimersByTime(UPDATE_RECHECK_MS); });
    await flush();
    expect(checkMock).toHaveBeenCalledTimes(2);
  });

  it('re-checks on window focus, at most once per 30 minutes', async () => {
    checkMock.mockResolvedValue(null);
    renderHook(() => useUpdateCheck());
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    expect(checkMock).toHaveBeenCalledTimes(1);
    await act(async () => { window.dispatchEvent(new Event('focus')); });
    expect(checkMock).toHaveBeenCalledTimes(1); // too soon
    await act(async () => { vi.advanceTimersByTime(FOCUS_RECHECK_MIN_MS); });
    await act(async () => { window.dispatchEvent(new Event('focus')); });
    await flush();
    expect(checkMock).toHaveBeenCalledTimes(2);
  });

  it('records a failed check instead of swallowing it, and clears it on success', async () => {
    checkMock.mockRejectedValueOnce('error sending request for url (https://github.com/...)');
    renderHook(() => useUpdateCheck());
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    expect(getUpdateStatus().lastError).toMatch(/error sending request/);
    expect(getUpdateStatus().lastCheckedAt).not.toBeNull();

    checkMock.mockResolvedValueOnce(null);
    await act(async () => { await requestUpdateCheck(); });
    expect(getUpdateStatus().lastError).toBeNull();
  });
});

describe('useUpdateCheck install', () => {
  it('reports download progress and relaunches after installing', async () => {
    const upd = fakeUpdate('1.0.4', async (cb) => {
      cb({ event: 'Started', data: { contentLength: 4_000_000 } });
      cb({ event: 'Progress', data: { chunkLength: 2_000_000 } });
      cb({ event: 'Progress', data: { chunkLength: 2_000_000 } });
      cb({ event: 'Finished' });
    });
    checkMock.mockResolvedValue(upd);
    const { result } = renderHook(() => useUpdateCheck());
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    expect(result.current.update?.version).toBe('1.0.4');

    // Pause the download half way to see what the banner is given.
    let finish: () => void = () => undefined;
    upd.downloadAndInstall.mockImplementationOnce((cb: (e: Ev) => void) => {
      cb({ event: 'Started', data: { contentLength: 4_000_000 } });
      cb({ event: 'Progress', data: { chunkLength: 2_000_000 } });
      return new Promise<void>((resolve) => { finish = resolve; });
    });
    let done: Promise<void> = Promise.resolve();
    await act(async () => { done = result.current.installUpdate(); });
    expect(result.current.installing).toBe(true);
    expect(result.current.progress).toEqual({ downloaded: 2_000_000, total: 4_000_000 });
    expect(relaunchMock).not.toHaveBeenCalled();
    await act(async () => { finish(); await done; });
    expect(upd.downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(checkMock).toHaveBeenCalledTimes(1); // installs the update it found, no second check
    expect(relaunchMock).toHaveBeenCalledTimes(1);
    expect(result.current.installError).toBeNull();
  });

  it('a failed install clears "installing", shows the reason, and never relaunches', async () => {
    const upd = fakeUpdate('1.0.4', () => Promise.reject('signature verification failed'));
    checkMock.mockResolvedValue(upd);
    const { result } = renderHook(() => useUpdateCheck());
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    await act(async () => { await result.current.installUpdate(); });
    expect(result.current.installing).toBe(false);
    expect(result.current.installError).toEqual({ stage: 'install', message: 'signature verification failed' });
    expect(relaunchMock).not.toHaveBeenCalled();
  });

  it('an installed update whose relaunch fails says to restart by hand', async () => {
    checkMock.mockResolvedValue(fakeUpdate('1.0.4'));
    relaunchMock.mockRejectedValueOnce(new Error('process plugin not allowed'));
    const { result } = renderHook(() => useUpdateCheck());
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    await act(async () => { await result.current.installUpdate(); });
    expect(result.current.installError).toEqual({ stage: 'restart', message: 'process plugin not allowed' });
    expect(result.current.installing).toBe(false);
  });

  it('a dismissed version is not shown again by the next periodic check', async () => {
    checkMock.mockResolvedValue(fakeUpdate('1.0.4'));
    const { result } = renderHook(() => useUpdateCheck());
    await act(async () => { vi.advanceTimersByTime(FIRST_CHECK_DELAY_MS); });
    await flush();
    act(() => result.current.dismiss());
    await act(async () => { vi.advanceTimersByTime(UPDATE_RECHECK_MS); });
    await flush();
    expect(result.current.update).toBeNull();
    checkMock.mockResolvedValue(fakeUpdate('1.0.5'));
    await act(async () => { await requestUpdateCheck(); });
    expect(result.current.update?.version).toBe('1.0.5');
  });
});

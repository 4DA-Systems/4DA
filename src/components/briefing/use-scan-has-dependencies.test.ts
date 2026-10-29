// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../../lib/commands', () => ({ cmd: (...args: unknown[]) => cmdMock(...args) }));

import { useScanHasDependencies } from './use-scan-has-dependencies';

beforeEach(() => cmdMock.mockReset());

// Fresh-profile audit E9: the "Scan my projects" nudge stayed on the Brief
// after a completed scan (it keyed off declared interests, which a scan never
// fills). A scan with stored dependencies hides it.
describe('useScanHasDependencies', () => {
  it('reports dependencies a scan stored, and asks again after a scan', async () => {
    cmdMock.mockResolvedValueOnce({ total_dependencies: 0 });
    const { result } = renderHook(() => useScanHasDependencies(true));
    await waitFor(() => expect(cmdMock).toHaveBeenCalledWith('ace_get_scan_summary'));
    expect(result.current[0]).toBe(false);

    cmdMock.mockResolvedValueOnce({ total_dependencies: 166 });
    act(() => result.current[1]());
    await waitFor(() => expect(result.current[0]).toBe(true));
  });

  it('does not ask while the nudge would not show anyway', () => {
    renderHook(() => useScanHasDependencies(false));
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('an unreadable summary keeps the nudge', async () => {
    cmdMock.mockRejectedValueOnce(new Error('ipc'));
    const { result } = renderHook(() => useScanHasDependencies(true));
    await waitFor(() => expect(cmdMock).toHaveBeenCalled());
    expect(result.current[0]).toBe(false);
  });
});

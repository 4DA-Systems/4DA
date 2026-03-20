/**
 * Tests for the type-safe IPC command layer (src/lib/commands.ts).
 *
 * Verifies that `cmd()` correctly delegates to Tauri's `invoke`,
 * forwards parameters, returns results, and propagates errors.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

// Must import AFTER mocks are set up
import { cmd } from '../../lib/commands';

describe('cmd()', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('calls invoke with correct command name', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    await cmd('get_analysis_status');
    expect(mockedInvoke).toHaveBeenCalledWith('get_analysis_status', {});
  });

  it('passes params to invoke', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    await cmd('add_interest', { topic: 'Rust' });
    expect(mockedInvoke).toHaveBeenCalledWith('add_interest', { topic: 'Rust' });
  });

  it('returns invoke result', async () => {
    const mockResult = { running: false, completed: true };
    mockedInvoke.mockResolvedValueOnce(mockResult);
    const result = await cmd('get_analysis_status');
    expect(result).toEqual(mockResult);
  });

  it('propagates invoke rejection', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('IPC failed'));
    await expect(cmd('get_analysis_status')).rejects.toThrow('IPC failed');
  });

  it('sends empty object when no params given', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    await cmd('cancel_analysis');
    expect(mockedInvoke).toHaveBeenCalledWith('cancel_analysis', {});
  });

  it('handles commands with optional params', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    await cmd('set_user_role', { role: null });
    expect(mockedInvoke).toHaveBeenCalledWith('set_user_role', { role: null });
  });

  it('defaults to empty object when params is undefined', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    // Calling the implementation overload directly with explicit undefined
    // to verify the `params ?? {}` fallback path
    await (cmd as (command: string, params?: unknown) => Promise<unknown>)(
      'get_settings',
      undefined,
    );
    expect(mockedInvoke).toHaveBeenCalledWith('get_settings', {});
  });

  it('handles known command names', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    // These should all type-check and invoke correctly
    await cmd('get_settings');
    await cmd('get_analysis_status');
    await cmd('run_cached_analysis');
    expect(mockedInvoke).toHaveBeenCalledTimes(3);
  });
});

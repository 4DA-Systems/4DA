// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The IPC timeout budget: a command the backend legitimately answers after
// the 30 s default must be on the long-running list, or the wrapper rejects
// a call the backend goes on to complete. Live 2026-09-07 (Blind Spots):
// `assess_blind_spots_with_ai` returned ok after 35,085 ms, the wrapper had
// already thrown at 30 s, and the tab showed "Assessment failed — try again"
// over a result the backend had cached for the next mount.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { cmd } from './commands';

const mockInvoke = vi.mocked(invoke);

/** A backend that answers only after `ms` — the shape of an LLM-backed command. */
function backendAnswersAfter(ms: number, value: unknown): void {
  mockInvoke.mockImplementationOnce(
    () => new Promise((resolve) => setTimeout(() => resolve(value), ms)),
  );
}

describe('IPC timeout budget', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockInvoke.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('an AI blind-spot triage that answers after the 30 s default still resolves', async () => {
    backendAnswersAfter(35_085, { model: 'test-model', assessments: [] });
    const settled = vi.fn();
    const call = cmd('assess_blind_spots_with_ai').then(
      (result) => {
        settled('ok');
        return result;
      },
      (error: unknown) => {
        settled('err');
        throw error;
      },
    );

    await vi.advanceTimersByTimeAsync(31_000);
    expect(settled, 'the wrapper must not give up before the backend answers').not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(5_000);
    await expect(call).resolves.toMatchObject({ model: 'test-model' });
    expect(settled).toHaveBeenCalledWith('ok');
  });

  it('a default command still gives up at 30 s', async () => {
    backendAnswersAfter(60_000, {});
    const outcome = cmd('get_settings').then(
      () => 'resolved',
      (error: Error) => error.message,
    );

    await vi.advanceTimersByTimeAsync(30_001);
    await expect(outcome).resolves.toMatch(/timed out after 30s/);
  });
});

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Review S4: the deferred brief's retry listener is armed once, removes
// itself after firing, and asks once more right after registering (the event
// may fire before the listener exists).

let handler: (() => void) | null = null;
const unlisten = vi.fn();
vi.mock('../../lib/tauri-events', () => ({
  safeListen: vi.fn((_event: string, cb: () => void) => {
    handler = cb;
    return Promise.resolve(unlisten);
  }),
}));

import { safeListen } from '../../lib/tauri-events';
import {
  INVENTORY_FRESH_EVENT,
  resetInventoryRetryForTests,
  retryBriefingWhenInventoryFresh,
} from '../briefing-slice';

const flush = () => new Promise((r) => setTimeout(r, 0));

describe('retryBriefingWhenInventoryFresh', () => {
  beforeEach(() => {
    handler = null;
    unlisten.mockReset();
    vi.mocked(safeListen).mockClear();
    resetInventoryRetryForTests();
  });

  it('arms one listener, re-asks once on registration, and fires once on the event', async () => {
    const generate = vi.fn(() => Promise.resolve());
    retryBriefingWhenInventoryFresh(generate, () => true);
    retryBriefingWhenInventoryFresh(generate, () => true); // already armed: no-op
    await flush();
    expect(safeListen).toHaveBeenCalledTimes(1);
    expect(vi.mocked(safeListen).mock.calls[0]?.[0]).toBe(INVENTORY_FRESH_EVENT);
    expect(generate).toHaveBeenCalledTimes(1); // the immediate re-ask

    handler?.();
    handler?.(); // a second event does nothing
    expect(generate).toHaveBeenCalledTimes(2);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('removes itself when the immediate re-ask is no longer deferred', async () => {
    const generate = vi.fn(() => Promise.resolve());
    retryBriefingWhenInventoryFresh(generate, () => false);
    await flush();
    await flush();
    expect(generate).toHaveBeenCalledTimes(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
    handler?.();
    expect(generate).toHaveBeenCalledTimes(1);
  });
});

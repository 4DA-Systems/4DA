// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';

import { WARM_THROTTLE_MS, resetSearchWarmForTest, warmSearchBackend } from './search-warm';

const calls: string[] = [];
let fail = false;

vi.mock('../../lib/commands', () => ({
  cmd: (name: string) => {
    calls.push(name);
    return fail ? Promise.reject(new Error('free tier')) : Promise.resolve(null);
  },
}));

beforeEach(() => {
  calls.length = 0;
  fail = false;
  resetSearchWarmForTest();
});

describe('warmSearchBackend', () => {
  it('sends warm_search when the palette opens', () => {
    expect(warmSearchBackend(1_000)).toBe(true);
    expect(calls).toEqual(['warm_search']);
  });

  it('throttles repeated opens inside the window', () => {
    warmSearchBackend(1_000);
    expect(warmSearchBackend(1_000 + WARM_THROTTLE_MS - 1)).toBe(false);
    expect(calls).toHaveLength(1);
    expect(warmSearchBackend(1_000 + WARM_THROTTLE_MS)).toBe(true);
    expect(calls).toHaveLength(2);
  });

  it('swallows a rejected warm (it is best effort)', async () => {
    fail = true;
    expect(warmSearchBackend(5_000)).toBe(true);
    await Promise.resolve();
    expect(calls).toEqual(['warm_search']);
  });
});

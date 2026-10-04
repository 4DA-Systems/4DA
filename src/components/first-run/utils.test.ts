// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { estimateRemainingSeconds } from './utils';

describe('estimateRemainingSeconds', () => {
  it('uses the source-count prior until any progress is reported', () => {
    expect(estimateRemainingSeconds(140, 0, 0)).toBe(140);
    expect(estimateRemainingSeconds(140, 20, 0)).toBe(120);
  });

  it('follows the observed rate once progress is substantial', () => {
    // 60s in, 50% done -> about 60s left, whatever the prior guessed.
    expect(estimateRemainingSeconds(300, 60, 0.5)).toBe(60);
    expect(estimateRemainingSeconds(30, 60, 0.5)).toBe(60);
  });

  it('blends prior and observed rate early on', () => {
    // 10s in, 10% done: observed 90s, prior 130s, weight 1/3 -> 117s.
    expect(estimateRemainingSeconds(140, 10, 0.1)).toBe(117);
  });

  it('a slower run than predicted raises the estimate instead of hitting zero', () => {
    expect(estimateRemainingSeconds(60, 120, 0.4)).toBe(180);
  });

  it('never goes negative and is zero when done', () => {
    expect(estimateRemainingSeconds(60, 500, 0)).toBe(0);
    expect(estimateRemainingSeconds(60, 30, 1)).toBe(0);
  });
});

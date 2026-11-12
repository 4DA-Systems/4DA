// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { dismissalDays, moveLegacyPreemptionDismissals, resetLegacyMoveForTests } from './alert-dismissals';

const cmdMock = vi.fn((..._a: unknown[]) => Promise.resolve());
vi.mock('./commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...a) }));

const LEGACY_KEY = 'preemption_dismissed';

describe('alert-dismissals', () => {
  beforeEach(() => {
    cmdMock.mockClear();
    localStorage.clear();
    resetLegacyMoveForTests();
  });

  // PR #1131 review: JSON that is not a list threw on every load, forever.
  it('clears an old entry that is not a list instead of throwing', async () => {
    localStorage.setItem(LEGACY_KEY, JSON.stringify({ id: 'x', ts: 1 }));
    await expect(moveLegacyPreemptionDismissals()).resolves.toBeUndefined();
    expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('clears an old entry that is not JSON at all', async () => {
    localStorage.setItem(LEGACY_KEY, '{not json');
    await moveLegacyPreemptionDismissals();
    expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  // Product decision 2026-11-13.
  it('a deliberate N/A or accepted risk holds 90 days; everything else 7', () => {
    expect(dismissalDays('not_applicable')).toBe(90);
    expect(dismissalDays('accepted_risk')).toBe(90);
    for (const a of ['acknowledged', 'snoozed', 'investigating', 'fixed']) expect(dismissalDays(a)).toBe(7);
  });
});

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach } from 'vitest';

import { recordPick, frecencyBoost, __resetFrecencyForTests } from './frecency';

const NOW = 1_000_000_000;
const HALF_LIFE_MS = 14 * 24 * 60 * 60 * 1000;

beforeEach(() => {
  localStorage.clear();
  __resetFrecencyForTests();
});

describe('frecency', () => {
  it('returns 0 for never-picked ids', () => {
    expect(frecencyBoost('never', NOW)).toBe(0);
  });

  it('boosts a picked id above zero', () => {
    recordPick('goto-results', NOW);
    expect(frecencyBoost('goto-results', NOW)).toBeGreaterThan(0);
  });

  it('grows with pick frequency', () => {
    recordPick('a', NOW);
    const once = frecencyBoost('a', NOW);
    recordPick('a', NOW);
    recordPick('a', NOW);
    recordPick('a', NOW);
    expect(frecencyBoost('a', NOW)).toBeGreaterThan(once);
  });

  it('decays with recency but stays positive within a half-life', () => {
    recordPick('a', NOW);
    const fresh = frecencyBoost('a', NOW);
    const aged = frecencyBoost('a', NOW + HALF_LIFE_MS);
    expect(aged).toBeLessThan(fresh);
    expect(aged).toBeGreaterThan(0);
  });

  it('stays bounded below 0.5 even after heavy use', () => {
    for (let i = 0; i < 100; i++) recordPick('a', NOW);
    expect(frecencyBoost('a', NOW)).toBeLessThan(0.5);
  });

  it('persists across an in-memory cache reset (localStorage-backed)', () => {
    recordPick('a', NOW);
    __resetFrecencyForTests(); // forces a reload from localStorage
    expect(frecencyBoost('a', NOW)).toBeGreaterThan(0);
  });
});

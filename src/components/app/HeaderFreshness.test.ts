// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { freshnessState, freshnessView, STALE_AFTER_MS } from './HeaderFreshness';

const NOW = Date.parse('2026-11-12T13:00:00Z');
const sync = (state: 'ready' | 'failed' | 'stale' | 'syncing' | 'no_inventory', synced_at: string | null) => ({
  state, ecosystems: 1, synced_at, error: null, in_flight: false, skipped_temp: 0,
});

describe('freshnessView', () => {
  // The engine writes completed_at as RFC 3339 with nanoseconds (live DB, 11-12).
  it('reads the engine receipt time and the SQLite advisory time', () => {
    const v = freshnessView({
      lastCycleAt: '2026-11-12T12:34:50.348319500+00:00',
      advisorySync: sync('ready', '2026-11-12 11:00:00'),
    }, NOW);
    expect(v.cycleAt?.toISOString()).toBe('2026-11-12T12:34:50.348Z');
    expect(v.advisoriesAt?.toISOString()).toBe('2026-11-12T11:00:00.000Z');
    expect(v.amber).toBe(false);
  });

  it('is amber past a day, and when the advisory check failed or never ran', () => {
    const old = new Date(NOW - STALE_AFTER_MS - 60_000).toISOString();
    expect(freshnessView({ lastCycleAt: old, advisorySync: sync('ready', '2026-11-12 11:00:00') }, NOW).amber).toBe(true);
    const fresh = '2026-11-12T12:00:00Z';
    expect(freshnessView({ lastCycleAt: fresh, advisorySync: sync('failed', '2026-11-12 11:00:00') }, NOW).amber).toBe(true);
    expect(freshnessView({ lastCycleAt: fresh, advisorySync: sync('ready', null) }, NOW).amber).toBe(true);
  });

  it('a check in progress, or a stack with nothing to check, is not a failure', () => {
    const fresh = '2026-11-12T12:00:00Z';
    expect(freshnessView({ lastCycleAt: fresh, advisorySync: sync('syncing', null) }, NOW).amber).toBe(false);
    expect(freshnessView({ lastCycleAt: fresh, advisorySync: sync('no_inventory', null) }, NOW).amber).toBe(false);
  });

  // PR #1131 review: a missing readiness dropped the advisory part with no
  // amber, and "stale" read as "check failed".
  it('no readiness at all is amber and "not checked", never fresh', () => {
    const v = freshnessView({ lastCycleAt: '2026-11-12T12:00:00Z', advisorySync: null }, NOW);
    expect(v.advisoryUnknown).toBe(true);
    expect(v.amber).toBe(true);
  });

  it('stale advisories are their own state, not a failed check', () => {
    const v = freshnessView({ lastCycleAt: '2026-11-12T12:00:00Z', advisorySync: sync('stale', '2026-11-08 11:00:00') }, NOW);
    expect(v.advisoryStale).toBe(true);
    expect(v.advisoryFailed).toBe(false);
    expect(v.amber).toBe(true);
  });

  // The live region announces this state; the minute-by-minute age must not
  // change it, or a screen reader is interrupted every minute.
  it('the announced state does not move while only the age ticks', () => {
    const at = (minutesAgo: number) => new Date(NOW - minutesAgo * 60_000).toISOString();
    const states = [5, 6, 7, 60, 300].map(m => freshnessState(freshnessView({
      lastCycleAt: at(m), advisorySync: sync('ready', '2026-11-12 11:00:00'),
    }, NOW), false));
    expect(new Set(states)).toEqual(new Set(['fresh']));
    const old = freshnessView({ lastCycleAt: at(25 * 60), advisorySync: sync('ready', '2026-11-12 11:00:00') }, NOW);
    expect(freshnessState(old, false)).toBe('stale');
    expect(freshnessState(old, true)).toBe('analyzing');
    expect(freshnessState(freshnessView({ lastCycleAt: null, advisorySync: null }, NOW), false)).toBe('notChecked');
    expect(freshnessState(freshnessView({ lastCycleAt: at(5), advisorySync: sync('failed', null) }, NOW), false)).toBe('checkFailed');
  });
});

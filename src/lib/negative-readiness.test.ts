// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';

import { preemptionNegative, sourceNewsReady } from './negative-readiness';
import type { AdvisorySync } from '../../src-tauri/bindings/bindings/AdvisorySync';
import type { DataFreshness } from '../../src-tauri/bindings/bindings/DataFreshness';

const sync = (over: Partial<AdvisorySync>): AdvisorySync => ({
  state: 'ready', ecosystems: 2, synced_at: '2026-10-12 06:00:00', error: null, ...over,
});

describe('preemptionNegative', () => {
  it('asserts only on ready', () => {
    expect(preemptionNegative(sync({}))).toEqual({ kind: 'assert' });
  });

  it('never synced and synced-after-compute both read as checking', () => {
    expect(preemptionNegative(sync({ state: 'syncing', synced_at: null }))).toEqual({ kind: 'checking' });
    expect(preemptionNegative(sync({ state: 'matching' }))).toEqual({ kind: 'checking' });
  });

  it('a failed sync carries its error', () => {
    expect(preemptionNegative(sync({ state: 'failed', error: 'npm: HTTP 503' })))
      .toEqual({ kind: 'failed', error: 'npm: HTTP 503' });
  });

  it('stale carries the sync time; no inventory is uncovered', () => {
    expect(preemptionNegative(sync({ state: 'stale' })))
      .toEqual({ kind: 'stale', syncedAt: '2026-10-12 06:00:00' });
    expect(preemptionNegative(sync({ state: 'no_inventory', ecosystems: 0 }))).toEqual({ kind: 'uncovered' });
  });

  it('a missing field (older backend) is silent, never an all-clear', () => {
    expect(preemptionNegative(undefined)).toEqual({ kind: 'silent' });
    expect(preemptionNegative(null)).toEqual({ kind: 'silent' });
  });
});

const fresh = (over: Partial<DataFreshness>): DataFreshness => ({
  newest_item_age_hours: 1, items_last_24h: 4, items_last_72h: 4,
  newest_source_check_age_hours: 0.5, source_checks_last_24h: 6, source_checks_last_72h: 6,
  failing_sources: 0, stale_sources: 0, total_sources: 6, is_stale: false, no_recent_fetches: false,
  last_run_items_scored: 4, last_run_sources_succeeded: 6, last_run_sources_failed: 0,
  last_run_age_minutes: 12, computed_at: null, ...over,
});

describe('sourceNewsReady', () => {
  it('ready on a handful of stack items: no item-count floor', () => {
    expect(sourceNewsReady(fresh({}))).toBe(true);
  });

  it('not ready without freshness, when stale, with no source check, or before an engine cycle', () => {
    expect(sourceNewsReady(null)).toBe(false);
    expect(sourceNewsReady(undefined)).toBe(false);
    expect(sourceNewsReady(fresh({ is_stale: true }))).toBe(false);
    expect(sourceNewsReady(fresh({ newest_source_check_age_hours: null }))).toBe(false);
    expect(sourceNewsReady(fresh({ source_checks_last_72h: 0 }))).toBe(false);
    expect(sourceNewsReady(fresh({ last_run_age_minutes: null }))).toBe(false);
  });
});

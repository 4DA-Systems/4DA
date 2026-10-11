// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import type { SourceHealthStatus } from '../../types/sources';

// Sources the user turned off in Settings (AD-054).
const turnedOff = new Set<string>();
vi.mock('../../config/sources', () => ({
  isSourceTurnedOff: (id: string) => turnedOff.has(id),
}));

let mockState: { sourceHealth: SourceHealthStatus[] } = { sourceHealth: [] };
vi.mock('../../store', () => ({
  useAppStore: (selector: (s: typeof mockState) => unknown) => selector(mockState),
}));

import { useColdStartGate } from '../use-cold-start-gate';

function row(source_type: string, items_fetched: number): SourceHealthStatus {
  return {
    source_type,
    status: 'healthy',
    last_success_relative: null,
    items_fetched,
    gap_message: null,
  };
}

function gate(): boolean {
  return renderHook(() => useColdStartGate()).result.current;
}

describe('useColdStartGate', () => {
  beforeEach(() => {
    turnedOff.clear();
    mockState = { sourceHealth: [] };
  });

  it('is cold with no source fetched yet (every interest off, no stack fetch)', () => {
    expect(gate()).toBe(true);
  });

  it('is cold below 100 fetched items and open at 100', () => {
    mockState = { sourceHealth: [row('osv', 40), row('cve', 59)] };
    expect(gate()).toBe(true);
    mockState = { sourceHealth: [row('osv', 40), row('cve', 60)] };
    expect(gate()).toBe(false);
  });

  it('does not count stale fetches from sources the user turned off', () => {
    // Live shape 2026-10-12: interests off but their frozen counts in the
    // payload (mastodon 397, rss 335) dwarfed the stack's own fetches.
    turnedOff.add('mastodon');
    turnedOff.add('rss');
    mockState = { sourceHealth: [row('osv', 7), row('cve', 75), row('mastodon', 397), row('rss', 335)] };
    expect(gate()).toBe(true);
  });

  it('opens on the stack sources alone once they carry enough data', () => {
    turnedOff.add('hackernews');
    mockState = {
      sourceHealth: [row('crates_io', 30), row('cve', 75), row('npm_registry', 34), row('hackernews', 112)],
    };
    expect(gate()).toBe(false);
  });

  it('counts an interest the user turned on', () => {
    mockState = { sourceHealth: [row('osv', 7), row('hackernews', 112)] };
    expect(gate()).toBe(false);
  });
});

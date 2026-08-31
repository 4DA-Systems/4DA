// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// AD-035: the latest briefing's verdicts bind display selection —
// demote-only, freshness-windowed, deterministic-security exempt.
import { describe, it, expect } from 'vitest';
import { activeBriefFilteredIds, isBriefSuppressed } from './brief-verdicts';
import type { BriefVerdicts } from '../store/types';

const NOW = 1_756_600_000_000;

function verdicts(overrides: Partial<BriefVerdicts> = {}): BriefVerdicts {
  return {
    filtered: { 42: 'self-promotional', 7: 'no stack relevance' },
    expiresAtMs: NOW + 60_000,
    ...overrides,
  };
}

describe('activeBriefFilteredIds', () => {
  it('returns the filtered ids while the verdicts are fresh', () => {
    const ids = activeBriefFilteredIds(verdicts(), NOW);
    expect([...ids].sort((a, b) => a - b)).toEqual([7, 42]);
  });

  it('returns the empty set once the freshness window closes', () => {
    // A stale briefing binds nothing — expiry is inclusive at the boundary.
    expect(activeBriefFilteredIds(verdicts({ expiresAtMs: NOW }), NOW).size).toBe(0);
    expect(activeBriefFilteredIds(verdicts({ expiresAtMs: NOW - 1 }), NOW).size).toBe(0);
  });

  it('returns the empty set for absent or empty verdicts (fail-open)', () => {
    expect(activeBriefFilteredIds(null, NOW).size).toBe(0);
    expect(activeBriefFilteredIds(undefined, NOW).size).toBe(0);
    expect(activeBriefFilteredIds(verdicts({ filtered: {} }), NOW).size).toBe(0);
  });
});

describe('isBriefSuppressed', () => {
  const active = activeBriefFilteredIds(verdicts(), NOW);

  it('suppresses a filtered item', () => {
    expect(isBriefSuppressed({ id: 42 }, active)).toBe(true);
  });

  it('never touches an item the briefing did not filter', () => {
    // The verdicts NEVER promote: a kept/unjudged item passes through with
    // no special treatment either way.
    expect(isBriefSuppressed({ id: 999 }, active)).toBe(false);
  });

  it('never suppresses deterministic security truth (is_critical_alert)', () => {
    expect(isBriefSuppressed({ id: 42, is_critical_alert: true }, active)).toBe(false);
  });

  it('suppresses nothing when no verdicts bind', () => {
    const none = activeBriefFilteredIds(null, NOW);
    expect(isBriefSuppressed({ id: 42 }, none)).toBe(false);
  });
});

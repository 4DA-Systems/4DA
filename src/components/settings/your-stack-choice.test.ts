// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect } from 'vitest';
import { applyStackChoice, isInactiveProject } from './your-stack-choice';
import type { StackProjectRow } from '../../lib/commands';

function row(partial: Partial<StackProjectRow>): StackProjectRow {
  return {
    path: 'd:/4da/victauri-gauntlet',
    name: 'victauri-gauntlet',
    dependency_count: 6,
    included: true,
    counts: false,
    forced: false,
    dormant: true,
    dormant_days: 161,
    scratch: true,
    ...partial,
  };
}

describe('Your Stack choices mirror the backend rule', () => {
  it('an inactive project is included but does not count until forced', () => {
    const r = row({});
    expect(isInactiveProject(r)).toBe(true);
    const forced = applyStackChoice(r, true, true);
    expect(forced).toMatchObject({ included: true, forced: true, counts: true });
    const undone = applyStackChoice(forced, false, true);
    expect(undone).toMatchObject({ included: true, forced: false, counts: false });
  });

  it('a plain toggle-off excludes and drops any force', () => {
    const r = applyStackChoice(row({ forced: true, counts: true }), false, false);
    expect(r).toMatchObject({ included: false, forced: false, counts: false });
  });

  it('a live project counts as soon as it is included', () => {
    const live = row({ dormant: false, scratch: false, dormant_days: 2, included: false, counts: false });
    expect(applyStackChoice(live, true, false)).toMatchObject({ included: true, counts: true });
  });
});

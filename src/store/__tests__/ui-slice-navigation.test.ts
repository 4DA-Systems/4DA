// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * ui-slice navigation test.
 *
 * Verifies that all 5 canonical views are navigable and invalid views
 * are rejected. Replaces the former tier-based navigation tests.
 */

import { describe, it, expect } from 'vitest';
import { createUiSlice } from '../ui-slice';

function makeHarness() {
  let state: Record<string, unknown> = {};
  const set = (patch: Record<string, unknown> | ((s: Record<string, unknown>) => Record<string, unknown>)) => {
    if (typeof patch === 'function') {
      state = { ...state, ...patch(state) };
    } else {
      state = { ...state, ...patch };
    }
  };
  const get = () => state as never;
  const slice = createUiSlice(set as never, get as never, undefined as never);
  state = { ...state, ...slice };
  return {
    get activeView() { return state.activeView; },
    setActiveView: slice.setActiveView,
  };
}

const VALID_VIEWS = ['briefing', 'preemption', 'blindspots', 'results', 'playbook'] as const;

describe('ui-slice navigation', () => {
  for (const view of VALID_VIEWS) {
    it(`navigates to "${view}"`, () => {
      const harness = makeHarness();
      harness.setActiveView(view);
      expect(harness.activeView).toBe(view);
    });
  }

  it('rejects removed views', () => {
    const harness = makeHarness();
    const removed = ['saved', 'toolkit', 'profile', 'calibrate', 'console', 'evidence'];
    for (const view of removed) {
      harness.setActiveView('briefing');
      // @ts-expect-error — testing runtime rejection of invalid views
      harness.setActiveView(view);
      expect(harness.activeView).toBe('briefing');
    }
  });

  it('defaults to briefing', () => {
    const harness = makeHarness();
    expect(harness.activeView).toBe('briefing');
  });
});

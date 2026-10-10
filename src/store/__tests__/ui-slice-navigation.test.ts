// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * ui-slice navigation test.
 *
 * Main nav is three views (AD-054: Brief · Preemption · Signal). Blind Spots
 * and Knowledge Gaps are Preemption sub-views; an old view id migrates to the
 * matching sub-view, and anything else is rejected.
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
    get preemptionSubView() { return state.preemptionSubView; },
    setActiveView: slice.setActiveView,
    setPreemptionSubView: slice.setPreemptionSubView,
    openPreemption: slice.openPreemption,
  };
}

const VALID_VIEWS = ['briefing', 'preemption', 'results'] as const;

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
    const removed = ['saved', 'toolkit', 'profile', 'calibrate', 'console', 'evidence', 'playbook', 'constructor', '__proto__'];
    for (const view of removed) {
      harness.setActiveView('briefing');
      // @ts-expect-error — testing runtime rejection of invalid views
      harness.setActiveView(view);
      expect(harness.activeView).toBe('briefing');
    }
  });

  it('defaults to Brief, with Preemption on its worklist', () => {
    const harness = makeHarness();
    expect(harness.activeView).toBe('briefing');
    expect(harness.preemptionSubView).toBe('worklist');
  });
});

describe('ui-slice AD-054 view migration', () => {
  it.each([
    ['blindspots', 'blindspots'],
    ['knowledge', 'knowledge'],
  ] as const)('old view "%s" lands on Preemption > %s', (legacy, sub) => {
    const harness = makeHarness();
    harness.setActiveView(legacy);
    expect(harness.activeView).toBe('preemption');
    expect(harness.preemptionSubView).toBe(sub);
  });

  it('selecting Preemption itself keeps the sub-view the user was on', () => {
    const harness = makeHarness();
    harness.setPreemptionSubView('knowledge');
    harness.setActiveView('briefing');
    harness.setActiveView('preemption');
    expect(harness.preemptionSubView).toBe('knowledge');
  });

  it('openPreemption selects the tab and the sub-view together', () => {
    const harness = makeHarness();
    harness.openPreemption('blindspots');
    expect(harness.activeView).toBe('preemption');
    expect(harness.preemptionSubView).toBe('blindspots');
  });

  it('rejects unknown sub-views', () => {
    const harness = makeHarness();
    // @ts-expect-error — testing runtime rejection
    harness.setPreemptionSubView('feed');
    // @ts-expect-error — testing runtime rejection
    harness.openPreemption('feed');
    expect(harness.activeView).toBe('briefing');
    expect(harness.preemptionSubView).toBe('worklist');
  });
});

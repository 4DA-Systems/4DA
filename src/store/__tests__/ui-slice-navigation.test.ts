// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * ui-slice navigation test.
 *
 * Regression guard for the 2026-04-11 bug where setActiveView silently
 * rejected 'preemption' and 'blindspots' because ui-slice.ts's internal
 * TIER_VIEWS constant hadn't been updated to include them.
 *
 * This test exercises the REAL ui-slice createUiSlice function (not a mock)
 * and verifies that every view in every tier's TIER_VIEWS is actually
 * navigable via setActiveView.
 */

import { describe, it, expect } from 'vitest';
import { createUiSlice, UI_SLICE_TIER_VIEWS } from '../ui-slice';
import { TIER_VIEWS as TABBAR_TIER_VIEWS } from '../../components/ViewTabBar';
import type { ViewTier } from '../types';

// Minimal store state harness — enough to exercise createUiSlice without
// pulling in all the other slices.
function makeHarness(initialTier: ViewTier = 'power') {
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
  state = { ...state, ...slice, viewTier: initialTier, showAllViews: false };
  return {
    get activeView() { return state.activeView; },
    get showAllViews() { return state.showAllViews; },
    setActiveView: slice.setActiveView,
  };
}

describe('ui-slice navigation', () => {
  const tiers: ViewTier[] = ['core', 'explorer', 'invested', 'power'];

  describe('TIER_VIEWS completeness', () => {
    it('UI_SLICE_TIER_VIEWS and TABBAR_TIER_VIEWS must have identical views per tier', () => {
      for (const tier of tiers) {
        const tabbarViews = [...TABBAR_TIER_VIEWS[tier]].sort();
        const sliceViews = [...UI_SLICE_TIER_VIEWS[tier]].sort();
        expect(sliceViews).toEqual(tabbarViews);
      }
    });
  });

  describe('setActiveView accepts all views listed in TIER_VIEWS for each tier', () => {
    for (const tier of tiers) {
      it(`tier "${tier}": every listed view must be navigable`, () => {
        const allowedViews = UI_SLICE_TIER_VIEWS[tier];
        const harness = makeHarness(tier);

        for (const view of allowedViews) {
          harness.setActiveView(view);
          expect(harness.activeView).toBe(view);
        }
      });
    }
  });

  describe('preemption and blindspots navigation', () => {
    it('preemption is navigable on explorer tier (was silently rejected before fix)', () => {
      const harness = makeHarness('explorer');
      harness.setActiveView('preemption');
      expect(harness.activeView).toBe('preemption');
    });

    it('blindspots is navigable on explorer tier (was silently rejected before fix)', () => {
      const harness = makeHarness('explorer');
      harness.setActiveView('blindspots');
      expect(harness.activeView).toBe('blindspots');
    });

    it('preemption is navigable on power tier', () => {
      const harness = makeHarness('power');
      harness.setActiveView('preemption');
      expect(harness.activeView).toBe('preemption');
    });

    it('blindspots is navigable on power tier', () => {
      const harness = makeHarness('power');
      harness.setActiveView('blindspots');
      expect(harness.activeView).toBe('blindspots');
    });

    it('preemption stays rejected on core tier (by design — progressive disclosure)', () => {
      const harness = makeHarness('core');
      const before = harness.activeView;
      harness.setActiveView('preemption');
      // Core tier excludes preemption, so the call is silently rejected.
      // Make sure activeView did NOT change to 'preemption'.
      expect(harness.activeView).toBe(before);
    });
  });
});

// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * TIER_VIEWS consistency test.
 *
 * The tier-to-views mapping exists in TWO places:
 *   1. src/components/ViewTabBar.tsx  — decides what tabs are VISIBLE
 *   2. src/store/ui-slice.ts           — decides what setActiveView ACCEPTS
 *
 * If they drift, you get the bug from 2026-04-11 where Preemption and
 * Blind Spots tabs showed up in the navbar but clicking them silently
 * failed because ui-slice.ts's tier check rejected the navigation.
 *
 * This test guarantees they stay in sync — forever.
 */

import { describe, it, expect } from 'vitest';
import { TIER_VIEWS as TABBAR_TIER_VIEWS } from '../ViewTabBar';
import { UI_SLICE_TIER_VIEWS } from '../../store/ui-slice';

describe('TIER_VIEWS consistency', () => {
  const tiers = ['core', 'explorer', 'invested', 'power'] as const;

  for (const tier of tiers) {
    it(`tier "${tier}" must have identical views in ViewTabBar and ui-slice`, () => {
      const tabbarViews = TABBAR_TIER_VIEWS[tier];
      const uiSliceViews = UI_SLICE_TIER_VIEWS[tier];

      // Sort both to compare as sets (order within a tier doesn't matter
      // for consistency — visual order is a separate concern).
      const tabbarSorted = [...tabbarViews].sort();
      const uiSliceSorted = [...uiSliceViews].sort();

      expect(uiSliceSorted).toEqual(tabbarSorted);
    });
  }

  it('every tier in ViewTabBar exists in ui-slice', () => {
    const tabbarTiers = Object.keys(TABBAR_TIER_VIEWS).sort();
    const uiSliceTiers = Object.keys(UI_SLICE_TIER_VIEWS).sort();
    expect(uiSliceTiers).toEqual(tabbarTiers);
  });

  it('tier progression is monotonic: each higher tier is a superset of the previous', () => {
    for (let i = 1; i < tiers.length; i++) {
      const prevTier = tiers[i - 1]!;
      const currTier = tiers[i]!;
      const lower = new Set(TABBAR_TIER_VIEWS[prevTier]);
      const higher = new Set(TABBAR_TIER_VIEWS[currTier]);
      for (const view of lower) {
        expect(higher.has(view)).toBe(true);
      }
    }
  });
});

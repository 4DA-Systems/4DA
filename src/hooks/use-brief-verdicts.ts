// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// AD-035 — store-connected accessor for the latest briefing's display-
// binding verdicts. The pure logic lives in `src/utils/brief-verdicts.ts`;
// this hook just subscribes to `briefVerdicts` (fetched by the briefing
// slice, cleared by its expiry timer) and memoizes the active id set.

import { useMemo } from 'react';
import { useAppStore } from '../store';
import { activeBriefFilteredIds } from '../utils/brief-verdicts';

export { isBriefSuppressed } from '../utils/brief-verdicts';

/** The item ids the latest fresh briefing filtered (empty when none bind). */
export function useActiveBriefFilteredIds(): ReadonlySet<number> {
  const briefVerdicts = useAppStore((s) => s.briefVerdicts);
  // The store clears briefVerdicts at expiry (timer in briefing-slice), so
  // memoizing on the object is sound: expiry arrives as a state change.
  return useMemo(() => activeBriefFilteredIds(briefVerdicts), [briefVerdicts]);
}

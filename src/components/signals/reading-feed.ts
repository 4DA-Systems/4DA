// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Signal's reading feed (AD-054 rule 3): news, social and editorial items from
// the interests the user turned on. Everything below the "Your stack" lane
// reads only these rows; registry releases and advisories are that lane's.

import { interestsEnabledCount, isSourceTurnedOff } from '../../config/sources';
import { useAppStore } from '../../store';
import type { SourceRelevance } from '../../types';
import { isStackFactRow } from './signal-lanes';

/** A row the reading feed may show: not a stack fact, not a turned-off source. */
export function isReadingRow(r: SourceRelevance): boolean {
  return !isStackFactRow(r) && !isSourceTurnedOff(r.source_type || 'hackernews');
}

/**
 * Whether any interest is on; `null` until source metadata loads. The store's
 * `sourceFilters` gets a new identity after that load and on every Settings
 * toggle, the only moments the count can change, so subscribing to it is
 * what re-renders the caller.
 */
export function useInterestsOn(): boolean | null {
  useAppStore((s) => s.sourceFilters);
  const count = interestsEnabledCount();
  return count === null ? null : count > 0;
}

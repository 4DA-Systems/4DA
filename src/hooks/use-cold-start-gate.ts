// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { isSourceTurnedOff } from '../config/sources';
import { useAppStore } from '../store';

/**
 * Intelligence Doctrine Rule 6: features requiring user data ship *silent*
 * until sufficient data arrives. Returns true when the system has not yet
 * collected enough source data to produce meaningful intelligence — panels
 * should render nothing (not a false "all clear") in this state.
 */
/**
 * Minimum total items fetched before the confident-negative panels (Preemption
 * "all clear", Knowledge Gaps "your knowledge is current", Blind Spots) may
 * assert anything. A single thin/partial/stale first fetch (totalFetched > 0 but
 * tiny) must NOT flip a day-one user into a false "you're all caught up" — that
 * is the exact banned pattern (doctrine #6). This gates only the EMPTY states;
 * real alerts/gaps (totalVisible > 0) always render regardless.
 */
const COLD_START_MIN_FETCHED = 100;

/**
 * Only sources the user reads count. A source turned off in Settings (AD-054)
 * keeps its last fetch count frozen in `source_health`; counting it would let
 * fetches 4DA no longer makes open the gate. `get_source_health_status` leaves
 * those rows out; the filter here keeps the gate honest against a backend
 * that still returns them.
 *
 * With every interest off and no stack fetch yet the list is empty, so the
 * gate stays closed: the panels stay silent on their negatives, and the
 * no-dependencies scan prompts (which do not read this gate) still onboard.
 */
export function useColdStartGate(): boolean {
  const sourceHealth = useAppStore((s) => s.sourceHealth);
  const totalFetched = sourceHealth
    .filter((s) => !isSourceTurnedOff(s.source_type))
    .reduce((sum, s) => sum + s.items_fetched, 0);
  return totalFetched < COLD_START_MIN_FETCHED;
}

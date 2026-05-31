// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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

export function useColdStartGate(): boolean {
  const sourceHealth = useAppStore((s) => s.sourceHealth);
  if (sourceHealth.length === 0) return true;
  const totalFetched = sourceHealth.reduce((sum, s) => sum + s.items_fetched, 0);
  return totalFetched < COLD_START_MIN_FETCHED;
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useAppStore } from '../store';

/**
 * Intelligence Doctrine Rule 6: features requiring user data ship *silent*
 * until sufficient data arrives. Returns true when the system has not yet
 * collected enough source data to produce meaningful intelligence — panels
 * should render nothing (not a false "all clear") in this state.
 */
export function useColdStartGate(): boolean {
  const sourceHealth = useAppStore((s) => s.sourceHealth);
  if (sourceHealth.length === 0) return true;
  const totalFetched = sourceHealth.reduce((sum, s) => sum + s.items_fetched, 0);
  return totalFetched === 0;
}

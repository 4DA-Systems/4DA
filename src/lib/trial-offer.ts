// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { TrialStatus } from '../store/types';

/**
 * What the app can truthfully say about the 14-day free trial.
 *
 * - `available`: the backend confirmed no trial has started and no licence
 *   is present, so `start_trial` will start one.
 * - `active`: the trial is running (the user already has Signal access).
 * - `used`: the trial ran and ended; it cannot be started again.
 * - `unavailable`: a licence is present, or the status has not loaded.
 */
export type TrialOffer = 'available' | 'active' | 'used' | 'unavailable';

/**
 * The trial starts by itself on first launch (settings/manager_init.rs), so
 * `available` is the rare case, and `start_trial` refuses every other one
 * (settings_commands_license.rs). A status that has not loaded is never
 * `available`: a paywall must not offer a trial it cannot confirm.
 */
export function trialOfferFor(status: TrialStatus | null | undefined): TrialOffer {
  if (!status) return 'unavailable';
  if (status.active) return 'active';
  if (status.started_at) return 'used';
  return status.has_license ? 'unavailable' : 'available';
}

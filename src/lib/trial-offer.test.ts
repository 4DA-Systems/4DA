// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The 14-day trial starts by itself on first launch, and `start_trial` refuses
// once a trial has run or a licence is present. Every paywall still showed
// "Start 14-Day Free Trial" whenever the trial status had not loaded, and the
// click did nothing. These cases pin when the offer may be made at all, using
// the shapes `get_trial_status` returns (gating.rs `get_trial_status_at`).
import { describe, expect, it } from 'vitest';
import { trialOfferFor } from './trial-offer';

describe('trialOfferFor', () => {
  it('never offers a trial it cannot confirm', () => {
    expect(trialOfferFor(null)).toBe('unavailable');
    expect(trialOfferFor(undefined)).toBe('unavailable');
  });

  it('a running trial is active, not on offer', () => {
    expect(trialOfferFor({ active: true, days_remaining: 9, started_at: '2026-11-06T00:00:00Z', has_license: false }))
      .toBe('active');
  });

  it('a trial that ran and ended is used', () => {
    expect(trialOfferFor({ active: false, days_remaining: 0, started_at: '2026-09-01T00:00:00Z', has_license: false }))
      .toBe('used');
  });

  it('an unparseable start stamp still counts as used (the backend refuses a restart)', () => {
    expect(trialOfferFor({ active: false, days_remaining: 0, started_at: 'not-a-date', has_license: false }))
      .toBe('used');
  });

  it('a licence opts out of the trial', () => {
    // A paid tier reports no stamp and has_license: true; start_trial answers
    // "Already have a license key".
    expect(trialOfferFor({ active: false, days_remaining: 0, started_at: null, has_license: true }))
      .toBe('unavailable');
  });

  it('is available only when the backend says none started and no licence exists', () => {
    expect(trialOfferFor({ active: false, days_remaining: 0, started_at: null, has_license: false }))
      .toBe('available');
  });
});

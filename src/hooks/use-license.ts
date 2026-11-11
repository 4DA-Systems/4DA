// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useAppStore } from '../store';
import { trialOfferFor } from '../lib/trial-offer';

export function useLicense() {
  const tier = useAppStore((s) => s.tier);
  const trialStatus = useAppStore((s) => s.trialStatus);
  const expired = useAppStore((s) => s.expired);
  const daysRemaining = useAppStore((s) => s.daysRemaining);
  const expiresAt = useAppStore((s) => s.expiresAt);
  const licenseLoaded = useAppStore((s) => s.licenseLoaded);
  const licenseLoadError = useAppStore((s) => s.licenseLoadError);
  const isPro = !expired && (tier === 'signal' || tier === 'team' || tier === 'enterprise' || tier === 'pro' || (trialStatus?.active === true));
  // `unverified` = we never got a successful license read AND the last attempt errored.
  // The tier badge uses this to show a "?" state instead of a confident (and possibly wrong)
  // "FREE", so a paid user whose cold-boot probe failed is never silently presented as Free.
  const licenseUnverified = !licenseLoaded && licenseLoadError !== null;
  // Whether a paywall may offer the free trial, say it has ended, or stay silent.
  const trialOffer = trialOfferFor(trialStatus);
  return { tier, isPro, trialStatus, trialOffer, expired, daysRemaining, expiresAt, licenseLoaded, licenseLoadError, licenseUnverified };
}

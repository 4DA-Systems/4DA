import { useAppStore } from '../store';

export function useLicense() {
  const tier = useAppStore((s) => s.tier);
  const trialStatus = useAppStore((s) => s.trialStatus);
  const expired = useAppStore((s) => s.expired);
  const daysRemaining = useAppStore((s) => s.daysRemaining);
  const expiresAt = useAppStore((s) => s.expiresAt);
  const isPro = !expired && (tier === 'pro' || tier === 'team' || (trialStatus?.active === true));
  return { tier, isPro, trialStatus, expired, daysRemaining, expiresAt };
}

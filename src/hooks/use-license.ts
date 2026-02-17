import { useAppStore } from '../store';

export function useLicense() {
  const tier = useAppStore((s) => s.tier);
  const trialStatus = useAppStore((s) => s.trialStatus);
  const isPro = tier === 'pro' || tier === 'team' || (trialStatus?.active === true);
  return { tier, isPro, trialStatus };
}

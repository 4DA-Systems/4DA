import { useAppStore } from '../store';

export function useLicense() {
  const tier = useAppStore((s) => s.tier);
  const isPro = tier === 'pro' || tier === 'team';
  return { tier, isPro };
}

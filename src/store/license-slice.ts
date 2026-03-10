import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { ProValueReport } from '../types';
import type { AppStore, LicenseSlice, TrialStatus } from './types';

export const createLicenseSlice: StateCreator<AppStore, [], [], LicenseSlice> = (set, get) => ({
  tier: 'free',
  licenseKey: '',
  licenseLoading: false,
  trialStatus: null,
  expiresAt: null,
  daysRemaining: 0,
  expired: false,
  proValueReport: null,

  loadLicense: async () => {
    try {
      const result = await cmd('get_license_tier');
      set({
        tier: result.expired ? 'free' : result.tier as 'free' | 'pro' | 'signal' | 'team' | 'enterprise',
        expiresAt: result.expires_at,
        daysRemaining: result.days_remaining,
        expired: result.expired,
      });
    } catch {
      set({ tier: 'free', expiresAt: null, daysRemaining: 0, expired: false });
    }
  },

  activateLicense: async (key: string): Promise<{ ok: boolean; reason?: string }> => {
    set({ licenseLoading: true });
    try {
      const result = await cmd('activate_license', {
        licenseKey: key,
      });
      if (result.success) {
        set({
          tier: result.tier as 'free' | 'pro' | 'signal' | 'team' | 'enterprise',
          licenseKey: key,
          licenseLoading: false,
          expired: false,
          expiresAt: result.expires_at ?? null,
          daysRemaining: result.expires_at ? 365 : 0,
        });
        // Also refresh STREETS tier in case this key has STREETS features
        get().loadStreetsTier?.();
        return { ok: true };
      }
      set({ licenseLoading: false });
      return { ok: false, reason: (result as unknown as { reason?: string }).reason ?? 'Validation failed' };
    } catch (e) {
      set({ licenseLoading: false });
      const msg = e instanceof Error ? e.message : typeof e === 'string' ? e : 'Unknown error';
      return { ok: false, reason: msg };
    }
  },

  loadTrialStatus: async () => {
    try {
      const status = await cmd('get_trial_status') as unknown as TrialStatus;
      set({ trialStatus: status });
    } catch {
      set({ trialStatus: null });
    }
  },

  startTrial: async () => {
    try {
      const result = await cmd('start_trial');
      if (result.success) {
        set({
          trialStatus: {
            active: true,
            days_remaining: result.days_remaining ?? 30,
            started_at: new Date().toISOString(),
            has_license: false,
          },
        });
        return true;
      }
      return false;
    } catch {
      return false;
    }
  },

  isPro: () => {
    const { tier, trialStatus, expired } = get();
    if (expired) return false;
    return tier === 'signal' || tier === 'team' || tier === 'enterprise' || tier === 'pro' || (trialStatus?.active === true);
  },

  loadProValueReport: async () => {
    try {
      const report = await cmd('get_pro_value_report') as unknown as ProValueReport;
      set({ proValueReport: report });
    } catch {
      // Silently ignore — badge just won't show
    }
  },
});

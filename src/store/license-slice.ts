import { invoke } from '@tauri-apps/api/core';
import type { StateCreator } from 'zustand';
import type { AppStore, LicenseSlice, TrialStatus } from './types';

export const createLicenseSlice: StateCreator<AppStore, [], [], LicenseSlice> = (set, get) => ({
  tier: 'free',
  licenseKey: '',
  licenseLoading: false,
  trialStatus: null,
  expiresAt: null,
  daysRemaining: 0,
  expired: false,

  loadLicense: async () => {
    try {
      const result = await invoke<{
        tier: string;
        has_key: boolean;
        activated_at: string | null;
        expires_at: string | null;
        days_remaining: number;
        expired: boolean;
      }>('get_license_tier');
      set({
        tier: result.expired ? 'free' : result.tier as 'free' | 'pro' | 'team',
        expiresAt: result.expires_at,
        daysRemaining: result.days_remaining,
        expired: result.expired,
      });
    } catch {
      set({ tier: 'free', expiresAt: null, daysRemaining: 0, expired: false });
    }
  },

  activateLicense: async (key: string) => {
    set({ licenseLoading: true });
    try {
      const result = await invoke<{ success: boolean; tier: string; expires_at?: string }>('activate_license', {
        licenseKey: key,
      });
      if (result.success) {
        set({
          tier: result.tier as 'free' | 'pro' | 'team',
          licenseKey: key,
          licenseLoading: false,
          expired: false,
          expiresAt: result.expires_at ?? null,
          daysRemaining: result.expires_at ? 365 : 0,
        });
        // Also refresh STREETS tier in case this key has STREETS features
        get().loadStreetsTier?.();
        return true;
      }
      set({ licenseLoading: false });
      return false;
    } catch {
      set({ licenseLoading: false });
      return false;
    }
  },

  loadTrialStatus: async () => {
    try {
      const status = await invoke<TrialStatus>('get_trial_status');
      set({ trialStatus: status });
    } catch {
      set({ trialStatus: null });
    }
  },

  startTrial: async () => {
    try {
      const result = await invoke<{ success: boolean; days_remaining?: number }>('start_trial');
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
    return tier === 'pro' || tier === 'team' || (trialStatus?.active === true);
  },
});

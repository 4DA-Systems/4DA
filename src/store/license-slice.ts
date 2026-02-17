import { invoke } from '@tauri-apps/api/core';
import type { StateCreator } from 'zustand';
import type { AppStore, LicenseSlice, TrialStatus } from './types';

export const createLicenseSlice: StateCreator<AppStore, [], [], LicenseSlice> = (set, get) => ({
  tier: 'free',
  licenseKey: '',
  licenseLoading: false,
  trialStatus: null,

  loadLicense: async () => {
    try {
      const result = await invoke<{
        tier: string;
        has_key: boolean;
        activated_at: string | null;
      }>('get_license_tier');
      set({ tier: result.tier as 'free' | 'pro' | 'team' });
    } catch {
      set({ tier: 'free' });
    }
  },

  activateLicense: async (key: string) => {
    set({ licenseLoading: true });
    try {
      const result = await invoke<{ success: boolean; tier: string }>('activate_license', {
        licenseKey: key,
      });
      if (result.success) {
        set({ tier: result.tier as 'free' | 'pro' | 'team', licenseKey: key, licenseLoading: false });
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
    const { tier, trialStatus } = get();
    return tier === 'pro' || tier === 'team' || (trialStatus?.active === true);
  },
});

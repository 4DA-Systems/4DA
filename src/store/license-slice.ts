import { invoke } from '@tauri-apps/api/core';
import type { StateCreator } from 'zustand';
import type { AppStore, LicenseSlice } from './types';

export const createLicenseSlice: StateCreator<AppStore, [], [], LicenseSlice> = (set, get) => ({
  tier: 'free',
  licenseKey: '',
  licenseLoading: false,

  loadLicense: async () => {
    try {
      const result = await invoke<{
        tier: string;
        has_key: boolean;
        activated_at: string | null;
      }>('get_license_tier');
      set({ tier: result.tier as 'free' | 'pro' | 'team' });
    } catch {
      // Default to free on error
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

  isPro: () => {
    const { tier } = get();
    return tier === 'pro' || tier === 'team';
  },
});

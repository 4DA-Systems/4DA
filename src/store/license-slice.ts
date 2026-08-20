// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import { isPlainBrowserRuntime } from '../lib/tauri-runtime';
import type { AppStore, LicenseSlice, TrialStatus } from './types';

export const createLicenseSlice: StateCreator<AppStore, [], [], LicenseSlice> = (set, get) => ({
  tier: 'free',
  licenseKey: '',
  licenseLoading: false,
  wasDowngraded: false,
  trialStatus: null,
  expiresAt: null,
  daysRemaining: 0,
  expired: false,
  // `licenseLoaded` flips true the first time get_license_tier returns successfully.
  // `licenseLoadError` holds the last transient-load failure message (null when healthy).
  // Together they let the UI distinguish a *confirmed* Free tier (loaded=true, tier='free')
  // from an *unverified* one (loaded=false, error set), so a paid user is never silently
  // shown Free just because the backend was still warming up at cold boot.
  licenseLoaded: false,
  licenseLoadError: null,

  loadLicense: async () => {
    if (isPlainBrowserRuntime()) {
      set({
        tier: 'free',
        expiresAt: null,
        daysRemaining: 0,
        expired: false,
        wasDowngraded: false,
        licenseLoaded: true,
        licenseLoadError: null,
      });
      return;
    }

    try {
      const result = await cmd('get_license_tier');
      const downgraded = (result as Record<string, unknown>).was_downgraded === true;
      set({
        tier: result.expired ? 'free' : result.tier as 'free' | 'pro' | 'signal' | 'team' | 'enterprise',
        expiresAt: result.expires_at,
        daysRemaining: result.days_remaining,
        expired: result.expired,
        wasDowngraded: downgraded,
        licenseLoaded: true,
        licenseLoadError: null,
      });
      if (downgraded) {
        console.warn('[4DA] License tier was downgraded to Free — key missing or expired.');
      }
    } catch (e) {
      // CRITICAL: a transient failure here (IPC timeout / backend still initialising at cold
      // boot) must NOT be treated as "this user is Free". Doing so silently drops paid users
      // into the Free experience for the whole session — the recurring Signal->Free badge bug.
      // Instead: keep whatever tier we already knew, flag the failure so the badge shows an
      // "unverified" (?) state, and let App.tsx retry with backoff.
      const msg = e instanceof Error ? e.message : typeof e === 'string' ? e : 'Unknown error';
      set({ licenseLoadError: msg });
      console.warn(
        '[4DA] Could not verify your license tier (transient load error — will retry). ' +
        'Your paid tier is NOT lost; the license is stored locally. ' +
        'If the badge keeps showing "?", open Settings -> License -> "Re-check licence", or restart the app. ' +
        'Detail: ' + msg,
      );
    }
  },

  activateLicense: async (
    key: string,
    fromDeepLink?: boolean,
  ): Promise<{ ok: boolean; reason?: string }> => {
    set({ licenseLoading: true });
    try {
      // Only send fromDeepLink when it is actually a deep-link activation, so the
      // manual-paste path sends exactly { licenseKey } (unchanged contract).
      const params: { licenseKey: string; fromDeepLink?: boolean } = { licenseKey: key };
      if (fromDeepLink) params.fromDeepLink = true;
      const result = await cmd('activate_license', params);
      if (result.success) {
        set({
          tier: result.tier as 'free' | 'pro' | 'signal' | 'team' | 'enterprise',
          licenseKey: key,
          licenseLoading: false,
          wasDowngraded: false,
          expired: false,
          expiresAt: result.expires_at ?? null,
          daysRemaining: result.expires_at
            ? Math.max(0, Math.ceil((new Date(result.expires_at).getTime() - Date.now()) / 86400000))
            : 0,
        });
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

  recoverLicenseByEmail: async (email: string): Promise<{ ok: boolean; reason?: string; tier?: string }> => {
    set({ licenseLoading: true });
    try {
      const result = await cmd('recover_license_by_email', { email });
      if (result.success) {
        set({
          tier: result.tier as 'free' | 'pro' | 'signal' | 'team' | 'enterprise',
          licenseKey: result.license_key ?? '',
          licenseLoading: false,
          wasDowngraded: false,
          expired: false,
          expiresAt: result.expires_at ?? null,
          daysRemaining: result.expires_at
            ? Math.max(0, Math.ceil((new Date(result.expires_at).getTime() - Date.now()) / 86400000))
            : 0,
        });
        return { ok: true, tier: result.tier };
      }
      set({ licenseLoading: false });
      return { ok: false, reason: result.reason ?? 'Unknown error' };
    } catch (e) {
      set({ licenseLoading: false });
      const msg = e instanceof Error ? e.message : typeof e === 'string' ? e : 'Unknown error';
      return { ok: false, reason: msg };
    }
  },

  loadTrialStatus: async () => {
    if (isPlainBrowserRuntime()) {
      set({ trialStatus: null });
      return;
    }

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
            days_remaining: result.days_remaining ?? 45,
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
});

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAppStore } from '../index';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const initialState = useAppStore.getState();

describe('license-slice', () => {
  beforeEach(() => {
    useAppStore.setState(initialState, true);
    vi.mocked(invoke).mockReset();
  });

  // ---------------------------------------------------------------------------
  // Initial state
  // ---------------------------------------------------------------------------
  describe('initial state', () => {
    it('has tier set to free', () => {
      expect(useAppStore.getState().tier).toBe('free');
    });

    it('has empty licenseKey', () => {
      expect(useAppStore.getState().licenseKey).toBe('');
    });

    it('has licenseLoading false', () => {
      expect(useAppStore.getState().licenseLoading).toBe(false);
    });

    it('has trialStatus null', () => {
      expect(useAppStore.getState().trialStatus).toBeNull();
    });

    it('has expiresAt null', () => {
      expect(useAppStore.getState().expiresAt).toBeNull();
    });

    it('has daysRemaining 0', () => {
      expect(useAppStore.getState().daysRemaining).toBe(0);
    });

    it('has expired false', () => {
      expect(useAppStore.getState().expired).toBe(false);
    });

    it('has proValueReport null', () => {
      expect(useAppStore.getState().proValueReport).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // loadLicense
  // ---------------------------------------------------------------------------
  describe('loadLicense', () => {
    it('loads license tier from backend', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({
        tier: 'pro',
        has_key: true,
        activated_at: '2024-01-01',
        expires_at: '2025-01-01',
        days_remaining: 180,
        expired: false,
      });

      await useAppStore.getState().loadLicense();

      expect(invoke).toHaveBeenCalledWith('get_license_tier');
      expect(useAppStore.getState().tier).toBe('pro');
      expect(useAppStore.getState().expiresAt).toBe('2025-01-01');
      expect(useAppStore.getState().daysRemaining).toBe(180);
      expect(useAppStore.getState().expired).toBe(false);
    });

    it('falls back to free when expired', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({
        tier: 'pro',
        has_key: true,
        activated_at: '2023-01-01',
        expires_at: '2024-01-01',
        days_remaining: 0,
        expired: true,
      });

      await useAppStore.getState().loadLicense();

      expect(useAppStore.getState().tier).toBe('free');
      expect(useAppStore.getState().expired).toBe(true);
    });

    it('resets to free on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadLicense();

      expect(useAppStore.getState().tier).toBe('free');
      expect(useAppStore.getState().expiresAt).toBeNull();
      expect(useAppStore.getState().daysRemaining).toBe(0);
      expect(useAppStore.getState().expired).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // activateLicense
  // ---------------------------------------------------------------------------
  describe('activateLicense', () => {
    it('activates license and updates state on success', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce({ success: true, tier: 'pro', expires_at: '2025-06-01' }) // activate_license
        .mockResolvedValueOnce({ tier: 'playbook', expired: false }); // loadStreetsTier gets called

      const result = await useAppStore.getState().activateLicense('PRO-KEY-123');

      expect(invoke).toHaveBeenCalledWith('activate_license', { licenseKey: 'PRO-KEY-123' });
      expect(result.ok).toBe(true);
      expect(useAppStore.getState().tier).toBe('pro');
      expect(useAppStore.getState().licenseKey).toBe('PRO-KEY-123');
      expect(useAppStore.getState().licenseLoading).toBe(false);
      expect(useAppStore.getState().expired).toBe(false);
      expect(useAppStore.getState().expiresAt).toBe('2025-06-01');
    });

    it('returns failure with reason when activation fails', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ success: false, reason: 'Invalid key (NOT_FOUND)' });

      const result = await useAppStore.getState().activateLicense('BAD-KEY');

      expect(result.ok).toBe(false);
      expect(result.reason).toBe('Invalid key (NOT_FOUND)');
      expect(useAppStore.getState().licenseLoading).toBe(false);
    });

    it('returns failure with error message on exception', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Network timeout'));

      const result = await useAppStore.getState().activateLicense('KEY');

      expect(result.ok).toBe(false);
      expect(result.reason).toBe('Network timeout');
      expect(useAppStore.getState().licenseLoading).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadTrialStatus
  // ---------------------------------------------------------------------------
  describe('loadTrialStatus', () => {
    it('loads trial status', async () => {
      const mockStatus = { active: true, days_remaining: 25, started_at: '2024-01-01', has_license: false };
      vi.mocked(invoke).mockResolvedValueOnce(mockStatus);

      await useAppStore.getState().loadTrialStatus();

      expect(invoke).toHaveBeenCalledWith('get_trial_status');
      expect(useAppStore.getState().trialStatus).toEqual(mockStatus);
    });

    it('sets trialStatus to null on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadTrialStatus();

      expect(useAppStore.getState().trialStatus).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // startTrial
  // ---------------------------------------------------------------------------
  describe('startTrial', () => {
    it('starts trial and updates status', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ success: true, days_remaining: 30 });

      const result = await useAppStore.getState().startTrial();

      expect(invoke).toHaveBeenCalledWith('start_trial');
      expect(result).toBe(true);
      expect(useAppStore.getState().trialStatus).not.toBeNull();
      expect(useAppStore.getState().trialStatus!.active).toBe(true);
      expect(useAppStore.getState().trialStatus!.days_remaining).toBe(30);
    });

    it('returns false when trial start fails', async () => {
      vi.mocked(invoke).mockResolvedValueOnce({ success: false });

      const result = await useAppStore.getState().startTrial();

      expect(result).toBe(false);
    });

    it('returns false on error', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      const result = await useAppStore.getState().startTrial();

      expect(result).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // isPro
  // ---------------------------------------------------------------------------
  describe('isPro', () => {
    it('returns true for pro tier', () => {
      useAppStore.setState({ tier: 'pro', expired: false });

      expect(useAppStore.getState().isPro()).toBe(true);
    });

    it('returns true for team tier', () => {
      useAppStore.setState({ tier: 'team', expired: false });

      expect(useAppStore.getState().isPro()).toBe(true);
    });

    it('returns true when trial is active', () => {
      useAppStore.setState({
        tier: 'free',
        trialStatus: { active: true, days_remaining: 10, started_at: '2024-01-01', has_license: false },
        expired: false,
      });

      expect(useAppStore.getState().isPro()).toBe(true);
    });

    it('returns false for free tier without trial', () => {
      useAppStore.setState({ tier: 'free', trialStatus: null, expired: false });

      expect(useAppStore.getState().isPro()).toBe(false);
    });

    it('returns false when expired', () => {
      useAppStore.setState({ tier: 'pro', expired: true });

      expect(useAppStore.getState().isPro()).toBe(false);
    });
  });

  // ---------------------------------------------------------------------------
  // loadProValueReport
  // ---------------------------------------------------------------------------
  describe('loadProValueReport', () => {
    it('loads the pro value report', async () => {
      const mockReport = { total_value: 120, features_used: 5, saved_hours: 10 };
      vi.mocked(invoke).mockResolvedValueOnce(mockReport);

      await useAppStore.getState().loadProValueReport();

      expect(invoke).toHaveBeenCalledWith('get_pro_value_report');
      expect(useAppStore.getState().proValueReport).toEqual(mockReport);
    });

    it('silently handles errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('fail'));

      await useAppStore.getState().loadProValueReport();

      expect(useAppStore.getState().proValueReport).toBeNull();
    });
  });
});

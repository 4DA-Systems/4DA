import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore } from './types';

// ============================================================================
// Types
// ============================================================================

export interface SunStatus {
  id: string;
  name: string;
  module_id: string;
  enabled: boolean;
  interval_secs: number;
  last_run: string | null;
  next_run_in_secs: number | null;
  last_result: string | null;
  run_count: number;
}

export interface SunAlert {
  id: number;
  sun_id: string;
  alert_type: string;
  message: string;
  acknowledged: boolean;
  created_at: string;
}

export interface SunRunResult {
  success: boolean;
  message: string;
  data: Record<string, unknown> | null;
}

export interface ModuleHealth {
  module_id: string;
  module_name: string;
  score: number;
  sun_count: number;
  success_rate: number;
  lessons_completed: number;
  total_lessons: number;
  last_activity: string | null;
}

export interface StreetHealthScore {
  overall: number;
  module_scores: ModuleHealth[];
  trend: string;
  top_action: string;
}

// ============================================================================
// Slice Interface
// ============================================================================

export interface SunsSlice {
  sunStatuses: SunStatus[];
  sunAlerts: SunAlert[];
  sunsLoading: boolean;
  sunsError: string | null;
  streetHealth: StreetHealthScore | null;
  streetHealthLoading: boolean;
  loadSunStatuses: () => Promise<void>;
  loadSunAlerts: () => Promise<void>;
  toggleSun: (sunId: string, enabled: boolean) => Promise<void>;
  acknowledgeSunAlert: (alertId: number) => Promise<void>;
  triggerSun: (sunId: string) => Promise<SunRunResult | null>;
  loadStreetHealth: () => Promise<void>;
}

// ============================================================================
// Slice Creator
// ============================================================================

let streetHealthInflight: Promise<void> | null = null;

export const createSunsSlice: StateCreator<AppStore, [], [], SunsSlice> = (set, get) => ({
  sunStatuses: [],
  sunAlerts: [],
  sunsLoading: false,
  sunsError: null,
  streetHealth: null,
  streetHealthLoading: false,

  loadSunStatuses: async () => {
    try {
      const statuses = await invoke<SunStatus[]>('get_sun_statuses');
      set({ sunStatuses: statuses });
    } catch (e) {
      set({ sunsError: String(e) });
    }
  },

  loadSunAlerts: async () => {
    try {
      const alerts = await invoke<SunAlert[]>('get_sun_alerts');
      set({ sunAlerts: alerts });
    } catch (e) {
      set({ sunsError: String(e) });
    }
  },

  toggleSun: async (sunId: string, enabled: boolean) => {
    try {
      await invoke('toggle_sun', { sunId, enabled });
      get().loadSunStatuses();
    } catch (e) {
      set({ sunsError: String(e) });
    }
  },

  acknowledgeSunAlert: async (alertId: number) => {
    try {
      await invoke('acknowledge_sun_alert', { alertId });
      get().loadSunAlerts();
    } catch (e) {
      set({ sunsError: String(e) });
    }
  },

  triggerSun: async (sunId: string) => {
    set({ sunsLoading: true });
    try {
      const result = await invoke<SunRunResult>('trigger_sun_manually', { sunId });
      get().loadSunStatuses();
      get().loadSunAlerts();
      set({ sunsLoading: false });
      return result;
    } catch (e) {
      set({ sunsError: String(e), sunsLoading: false });
      return null;
    }
  },

  loadStreetHealth: async () => {
    if (streetHealthInflight) return streetHealthInflight;
    const doLoad = async () => {
      set({ streetHealthLoading: true });
      try {
        const health = await invoke<StreetHealthScore>('get_street_health');
        set({ streetHealth: health, streetHealthLoading: false });
      } catch (e) {
        set({ sunsError: String(e), streetHealthLoading: false });
      } finally {
        streetHealthInflight = null;
      }
    };
    streetHealthInflight = doLoad();
    return streetHealthInflight;
  },
});

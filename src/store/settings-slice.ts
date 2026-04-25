// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore, SettingsSlice, SettingsForm, OllamaStatus } from './types';
import { normalizeOllamaStatus } from '../utils/normalize-ollama';
import { translateError } from '../utils/error-messages';
import { setActivityTrackingEnabled } from '../hooks/use-telemetry';

const defaultSettingsForm: SettingsForm = {
  provider: 'anthropic',
  apiKey: '',
  model: 'claude-haiku-4-5-20251001',
  baseUrl: '',
  rerankEnabled: false,
  maxItems: 15,
  minScore: 0.25,
  dailyTokenLimit: 100000,
  dailyCostLimit: 50,
};

let onboardingChecked = false;

export const createSettingsSlice: StateCreator<AppStore, [], [], SettingsSlice> = (set, get) => ({
  settings: null,
  settingsForm: { ...defaultSettingsForm },
  settingsStatus: '',
  showOnboarding: false,
  ollamaStatus: null,
  ollamaModels: [],
  modelRegistry: null,

  setSettingsForm: (partial) => {
    set(state => ({
      settingsForm: { ...state.settingsForm, ...partial },
    }));
  },

  setSettingsFormFull: (updaterOrValue) => {
    set(state => ({
      settingsForm: typeof updaterOrValue === 'function'
        ? updaterOrValue(state.settingsForm)
        : updaterOrValue,
    }));
  },

  setSettingsStatus: (status) => set({ settingsStatus: status }),
  setShowOnboarding: (show) => set({ showOnboarding: show }),

  loadSettings: async () => {
    try {
      const s = await cmd('get_settings');
      // Activity-tracking gate. No telemetry is recorded until this
      // line flips the runtime flag. Default (no settings, bootstrap
      // failure, user not opted in) keeps it off. See use-telemetry.ts.
      const priv = (s as unknown as { privacy?: { activity_tracking_opt_in?: boolean } }).privacy;
      setActivityTrackingEnabled(priv?.activity_tracking_opt_in === true);
      set(state => ({
        settings: s,
        settingsForm: {
          ...state.settingsForm,
          provider: s.llm.provider !== 'none' ? s.llm.provider : 'anthropic',
          model: s.llm.model || 'claude-haiku-4-5-20251001',
          baseUrl: s.llm.base_url || '',
          rerankEnabled: s.rerank.enabled,
          maxItems: s.rerank.max_items_per_batch,
          minScore: s.rerank.min_embedding_score,
          dailyTokenLimit: s.rerank.daily_token_limit,
          dailyCostLimit: s.rerank.daily_cost_limit_cents,
        },
      }));

      if (!onboardingChecked) {
        onboardingChecked = true;
        if (!(s as unknown as Record<string, unknown>).onboarding_complete) {
          set({ showOnboarding: true });
        }
      }

      // Load model registry (non-blocking)
      cmd('get_model_registry').then((registry) => {
        set({ modelRegistry: registry });
      }).catch((e) => console.debug('[settings] model registry:', e));
    } catch {
      /* settings not available */
    }
  },

  saveSettings: async () => {
    const { settingsForm, loadSettings } = get();
    set({ settingsStatus: 'Saving...' });

    // Trim and validate API key before saving
    const trimmedApiKey = (settingsForm.apiKey || '').trim();
    if (trimmedApiKey.length > 0 && trimmedApiKey.length < 20) {
      set({ settingsStatus: 'Error: API key is too short (must be at least 20 characters)' });
      return;
    }

    const isCloud = settingsForm.provider !== 'ollama' && settingsForm.provider !== 'local';
    try {
      await Promise.all([
        cmd('set_llm_provider', {
          provider: settingsForm.provider,
          apiKey: trimmedApiKey,
          model: settingsForm.model,
          baseUrl: settingsForm.baseUrl || null,
        }),
        cmd('set_rerank_config', {
          enabled: settingsForm.rerankEnabled,
          maxItems: settingsForm.maxItems,
          minScore: settingsForm.minScore,
          dailyTokenLimit: settingsForm.dailyTokenLimit,
          dailyCostLimit: settingsForm.dailyCostLimit,
        }),
        ...(isCloud ? [cmd('set_privacy_config', { cloudLlmDisclosureAccepted: true })] : []),
      ]);

      set({ settingsStatus: 'Settings saved!' });
      await loadSettings();
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      set({ settingsStatus: `Error: ${translateError(error)}` });
    }
  },

  testConnection: async () => {
    const { saveSettings, settingsForm } = get();
    const isOllama = settingsForm.provider === 'ollama';
    set({ settingsStatus: isOllama ? 'Testing Ollama connection...' : 'Testing connection...' });
    try {
      await saveSettings();

      const timeoutMs = isOllama ? 90_000 : 30_000;
      const testPromise = cmd('test_llm_connection');
      const timeoutPromise = new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(
          isOllama
            ? 'Ollama did not respond in time. Try restarting Ollama or using a smaller model.'
            : 'Connection timed out. Check your internet connection.',
        )), timeoutMs),
      );

      const result = await Promise.race([testPromise, timeoutPromise]);
      set({ settingsStatus: result.message });
    } catch (error) {
      set({ settingsStatus: `Connection failed: ${translateError(error)}` });
    }
  },

  checkOllamaStatus: async (baseUrl?: string) => {
    try {
      const raw = await cmd('check_ollama_status', { baseUrl: baseUrl ?? null }) as unknown as Record<string, unknown>;
      const status = normalizeOllamaStatus(raw);
      set({ ollamaStatus: status });
      if (status.running && status.models.length > 0) {
        set({ ollamaModels: status.models });
      }
      return status;
    } catch (error) {
      console.error('Failed to check Ollama status:', error);
      const errorStatus: OllamaStatus = {
        running: false,
        version: null,
        models: [],
        base_url: baseUrl || 'http://localhost:11434',
        error: String(error),
      };
      set({ ollamaStatus: errorStatus });
      return errorStatus;
    }
  },

  refreshModelRegistry: async () => {
    try {
      await cmd('refresh_model_registry');
      const registry = await cmd('get_model_registry');
      set({ modelRegistry: registry });
    } catch (error) {
      console.error('Failed to refresh model registry:', error);
    }
  },
});

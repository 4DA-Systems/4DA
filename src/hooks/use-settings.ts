import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Settings } from '../types';

interface SettingsForm {
  provider: string;
  apiKey: string;
  model: string;
  baseUrl: string;
  rerankEnabled: boolean;
  maxItems: number;
  minScore: number;
  dailyTokenLimit: number;
  dailyCostLimit: number;
}

export interface OllamaStatus {
  running: boolean;
  version: string | null;
  models: string[];
  base_url: string;
  error?: string;
}

const defaultSettingsForm: SettingsForm = {
  provider: 'anthropic',
  apiKey: '',
  model: 'claude-3-haiku-20240307',
  baseUrl: '',
  rerankEnabled: false,
  maxItems: 15,
  minScore: 0.25,
  dailyTokenLimit: 100000,
  dailyCostLimit: 50,
};

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [settingsForm, setSettingsForm] = useState<SettingsForm>(defaultSettingsForm);
  const [settingsStatus, setSettingsStatus] = useState('');
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const onboardingChecked = useRef(false);

  const loadSettings = useCallback(async () => {
    try {
      const s = await invoke<Settings>('get_settings');
      setSettings(s);
      setSettingsForm((f) => ({
        ...f,
        provider: s.llm.provider !== 'none' ? s.llm.provider : 'anthropic',
        model: s.llm.model || 'claude-3-haiku-20240307',
        baseUrl: s.llm.base_url || '',
        rerankEnabled: s.rerank.enabled,
        maxItems: s.rerank.max_items_per_batch,
        minScore: s.rerank.min_embedding_score,
        dailyTokenLimit: s.rerank.daily_token_limit,
        dailyCostLimit: s.rerank.daily_cost_limit_cents,
      }));

      // Check if onboarding is needed (first run) - only check once per session
      if (!onboardingChecked.current) {
        onboardingChecked.current = true;
        const rawSettings = await invoke<Record<string, unknown>>('get_settings');
        if (!rawSettings.onboarding_complete) {
          setShowOnboarding(true);
        }
      }
    } catch (error) {
      console.debug('Settings not available:', error);
    }
  }, []);

  const saveSettings = useCallback(async () => {
    setSettingsStatus('Saving...');
    try {
      await invoke('set_llm_provider', {
        provider: settingsForm.provider,
        apiKey: settingsForm.apiKey || '',
        model: settingsForm.model,
        baseUrl: settingsForm.baseUrl || null,
      });

      await invoke('set_rerank_config', {
        enabled: settingsForm.rerankEnabled,
        maxItems: settingsForm.maxItems,
        minScore: settingsForm.minScore,
        dailyTokenLimit: settingsForm.dailyTokenLimit,
        dailyCostLimit: settingsForm.dailyCostLimit,
      });

      setSettingsStatus('Settings saved!');
      await loadSettings();
      setTimeout(() => setSettingsStatus(''), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  }, [settingsForm, loadSettings]);

  const testConnection = useCallback(async () => {
    setSettingsStatus('Testing connection...');
    try {
      await saveSettings();
      const result = await invoke<{ success: boolean; message: string }>('test_llm_connection');
      setSettingsStatus(result.message);
    } catch (error) {
      setSettingsStatus(`Connection failed: ${error}`);
    }
  }, [saveSettings]);

  const checkOllamaStatus = useCallback(async (baseUrl?: string) => {
    try {
      const status = await invoke<OllamaStatus>('check_ollama_status', { baseUrl });
      setOllamaStatus(status);
      if (status.running && status.models.length > 0) {
        setOllamaModels(status.models);
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
      setOllamaStatus(errorStatus);
      return errorStatus;
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  return {
    settings,
    settingsForm,
    setSettingsForm,
    settingsStatus,
    setSettingsStatus,
    showOnboarding,
    setShowOnboarding,
    loadSettings,
    saveSettings,
    testConnection,
    ollamaStatus,
    ollamaModels,
    checkOllamaStatus,
  };
}

import { useEffect } from 'react';
import { useAppStore } from '../store';

export type { OllamaStatus, SettingsForm } from '../store';

/**
 * Settings hook — thin wrapper around Zustand store.
 * All state lives in the store; this hook adds the init-load effect.
 */
export function useSettings() {
  const settings = useAppStore(s => s.settings);
  const settingsForm = useAppStore(s => s.settingsForm);
  const settingsStatus = useAppStore(s => s.settingsStatus);
  const setSettingsStatus = useAppStore(s => s.setSettingsStatus);
  const showOnboarding = useAppStore(s => s.showOnboarding);
  const setShowOnboarding = useAppStore(s => s.setShowOnboarding);
  const loadSettings = useAppStore(s => s.loadSettings);
  const saveSettings = useAppStore(s => s.saveSettings);
  const testConnection = useAppStore(s => s.testConnection);
  const ollamaStatus = useAppStore(s => s.ollamaStatus);
  const ollamaModels = useAppStore(s => s.ollamaModels);
  const checkOllamaStatus = useAppStore(s => s.checkOllamaStatus);
  const setSettingsForm = useAppStore(s => s.setSettingsFormFull);

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

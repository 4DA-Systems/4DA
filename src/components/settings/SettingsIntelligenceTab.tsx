// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { AIProviderSection } from './AIProviderSection';
import { LicenseSection } from './LicenseSection';
import { ProValuePanel } from '../ProValuePanel';

import type { Settings } from '../../types';
import type { OllamaStatus } from '../../hooks/use-settings';
import type { ModelRegistryData } from '../../store/types';
import type { SettingsForm } from './ai-provider-types';

interface SettingsIntelligenceTabProps {
  settings: Settings | null;
  settingsForm: SettingsForm;
  setSettingsForm: React.Dispatch<React.SetStateAction<SettingsForm>>;
  ollamaStatus: OllamaStatus | null;
  ollamaModels: string[];
  checkOllamaStatus: (baseUrl?: string) => void;
  modelRegistry: ModelRegistryData | null;
  onRefreshRegistry: () => void;
  setSettingsStatus: (s: string) => void;
  saveSettings: () => void;
  testConnection: () => void;
}

type ButtonState = 'idle' | 'loading' | 'success' | 'error';

export const SettingsIntelligenceTab = memo(function SettingsIntelligenceTab({
  settings,
  settingsForm,
  setSettingsForm,
  ollamaStatus,
  ollamaModels,
  checkOllamaStatus,
  modelRegistry,
  onRefreshRegistry,
  setSettingsStatus,
  saveSettings,
  testConnection,
}: SettingsIntelligenceTabProps) {
  const { t } = useTranslation();
  const [saveState, setSaveState] = useState<ButtonState>('idle');
  const [testState, setTestState] = useState<ButtonState>('idle');
  const [inlineStatus, setInlineStatus] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  const handleSave = useCallback(async () => {
    setSaveState('loading');
    setInlineStatus(null);
    try {
      await saveSettings();
      setSaveState('success');
      setInlineStatus({ type: 'success', message: t('settings.ai.settingsSaved') });
      setTimeout(() => { setSaveState('idle'); setInlineStatus(null); }, 3000);
    } catch {
      setSaveState('error');
      setInlineStatus({ type: 'error', message: t('settings.ai.saveFailed') });
      setTimeout(() => { setSaveState('idle'); setInlineStatus(null); }, 4000);
    }
  }, [saveSettings, t]);

  const handleTest = useCallback(async () => {
    setTestState('loading');
    setInlineStatus(null);
    try {
      await testConnection();
      setTestState('success');
      setTimeout(() => setTestState('idle'), 3000);
    } catch {
      setTestState('error');
      setTimeout(() => setTestState('idle'), 4000);
    }
  }, [testConnection]);

  return (
    <div id="tabpanel-intelligence" role="tabpanel">
      <div className="space-y-4">
        <PanelErrorBoundary name="AI Provider">
          <AIProviderSection
            settings={settings}
            settingsForm={settingsForm}
            setSettingsForm={setSettingsForm}
            ollamaStatus={ollamaStatus}
            ollamaModels={ollamaModels}
            checkOllamaStatus={checkOllamaStatus}
            modelRegistry={modelRegistry}
            onRefreshRegistry={onRefreshRegistry}
          />
        </PanelErrorBoundary>

        {/* Inline status feedback — always visible near the buttons */}
        {inlineStatus && (
          <div className={`text-sm p-3 rounded-lg border ${
            inlineStatus.type === 'success'
              ? 'bg-green-500/10 text-green-400 border-green-500/30'
              : 'bg-red-500/10 text-red-400 border-red-500/30'
          }`}>
            {inlineStatus.message}
          </div>
        )}

        {/* AI Configuration Actions */}
        <div className="flex gap-3">
          <button
            onClick={handleSave}
            disabled={saveState === 'loading'}
            aria-label={t('settings.ai.saveConfiguration')}
            className={`flex-1 px-4 py-2.5 text-sm font-medium rounded-lg transition-all shadow-lg flex items-center justify-center gap-2 ${
              saveState === 'success'
                ? 'bg-green-600 text-white shadow-green-500/20'
                : saveState === 'error'
                  ? 'bg-red-600 text-white shadow-red-500/20'
                  : 'bg-gradient-to-r from-orange-500 to-orange-600 text-white hover:from-orange-600 hover:to-orange-700 shadow-orange-500/20'
            } disabled:opacity-60`}
          >
            {saveState === 'loading' && (
              <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            )}
            {saveState === 'success' ? t('settings.ai.saved') : saveState === 'error' ? t('settings.ai.saveFailed') : t('settings.ai.saveConfiguration')}
          </button>
          <button
            onClick={handleTest}
            disabled={testState === 'loading'}
            aria-label={t('settings.testConnection')}
            className={`px-6 py-2.5 text-sm border rounded-lg transition-all flex items-center gap-2 ${
              testState === 'success'
                ? 'bg-green-500/10 text-green-400 border-green-500/30'
                : testState === 'error'
                  ? 'bg-red-500/10 text-red-400 border-red-500/30'
                  : 'bg-bg-tertiary text-text-secondary border-border hover:text-white hover:border-orange-500/30'
            } disabled:opacity-60`}
          >
            {testState === 'loading' && (
              <span className="w-4 h-4 border-2 border-current/30 border-t-current rounded-full animate-spin" />
            )}
            {testState === 'success' ? t('settings.ai.connectionOk') : testState === 'error' ? t('settings.ai.connectionFailed') : t('settings.testConnection')}
          </button>
        </div>

        <PanelErrorBoundary name="License">
          <LicenseSection onStatus={setSettingsStatus} />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Signal Value">
          <ProValuePanel />
        </PanelErrorBoundary>
      </div>
    </div>
  );
});

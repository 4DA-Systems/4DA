// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { AIProviderSection } from './AIProviderSection';
import { LicenseSection } from './LicenseSection';

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

        {/* Intelligence Engine — read-only info about built-in models */}
        <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
          <div className="flex items-center gap-3 mb-3">
            <div className="w-8 h-8 bg-purple-500/20 rounded-lg flex items-center justify-center">
              <svg className="w-4 h-4 text-purple-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 0 1-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 0 1 4.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0 1 12 15a9.065 9.065 0 0 0-6.23.693L5 14.5m14.8.8 1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A48.309 48.309 0 0 1 12 21c-2.773 0-5.491-.235-8.135-.687-1.718-.293-2.3-2.379-1.067-3.61L5 14.5" />
              </svg>
            </div>
            <div>
              <h3 className="text-sm font-medium text-text-primary">{t('settings.ai.intelligenceEngine')}</h3>
              <p className="text-xs text-text-muted">{t('settings.ai.intelligenceEngineDesc')}</p>
            </div>
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between px-3 py-2 bg-bg-secondary rounded-lg">
              <span className="text-xs text-text-muted">{t('settings.ai.engineEmbedding')}</span>
              <span className="text-xs text-purple-300 font-mono">{t('settings.ai.engineEmbeddingValue')}</span>
            </div>
            <div className="flex items-center justify-between px-3 py-2 bg-bg-secondary rounded-lg">
              <span className="text-xs text-text-muted">{t('settings.ai.engineReranker')}</span>
              <span className="text-xs text-purple-300 font-mono">{t('settings.ai.engineRerankerValue')}</span>
            </div>
            <div className="flex items-center justify-between px-3 py-2 bg-bg-secondary rounded-lg">
              <span className="text-xs text-text-muted">{t('settings.ai.engineTopics')}</span>
              <span className="text-xs text-purple-300 font-mono">{t('settings.ai.engineTopicsValue', { count: 100 })}</span>
            </div>
          </div>
        </div>

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
            onClick={() => { void handleSave(); }}
            disabled={saveState === 'loading'}
            aria-label={t('settings.ai.saveConfiguration')}
            className={`flex-1 px-4 py-2.5 text-sm font-medium rounded-lg transition-all shadow-lg flex items-center justify-center gap-2 ${
              saveState === 'success'
                ? 'bg-green-600 text-text-primary shadow-green-500/20'
                : saveState === 'error'
                  ? 'bg-red-600 text-text-primary shadow-red-500/20'
                  : 'bg-gradient-to-r from-orange-500 to-orange-600 text-text-primary hover:from-orange-600 hover:to-orange-700 shadow-orange-500/20'
            } disabled:opacity-60`}
          >
            {saveState === 'loading' && (
              <span className="w-4 h-4 border-2 border-text-primary/30 border-t-text-primary rounded-full animate-spin" />
            )}
            {saveState === 'success' ? t('settings.ai.saved') : saveState === 'error' ? t('settings.ai.saveFailed') : t('settings.ai.saveConfiguration')}
          </button>
          <button
            onClick={() => { void handleTest(); }}
            disabled={testState === 'loading'}
            aria-label={t('settings.testConnection')}
            className={`px-6 py-2.5 text-sm border rounded-lg transition-all flex items-center gap-2 ${
              testState === 'success'
                ? 'bg-green-500/10 text-green-400 border-green-500/30'
                : testState === 'error'
                  ? 'bg-red-500/10 text-red-400 border-red-500/30'
                  : 'bg-bg-tertiary text-text-secondary border-border hover:text-text-primary hover:border-orange-500/30'
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
      </div>
    </div>
  );
});

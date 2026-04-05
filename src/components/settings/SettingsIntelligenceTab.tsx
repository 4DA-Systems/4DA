import { memo } from 'react';
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

        {/* AI Configuration Actions */}
        <div className="flex gap-3">
          <button
            onClick={saveSettings}
            aria-label={t('settings.ai.saveConfiguration')}
            className="flex-1 px-4 py-2.5 text-sm bg-gradient-to-r from-orange-500 to-orange-600 text-white font-medium rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all shadow-lg shadow-orange-500/20"
          >
            {t('settings.ai.saveConfiguration')}
          </button>
          <button
            onClick={testConnection}
            aria-label={t('settings.testConnection')}
            className="px-6 py-2.5 text-sm bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
          >
            {t('settings.testConnection')}
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

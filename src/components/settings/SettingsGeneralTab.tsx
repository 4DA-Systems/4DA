import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { LocaleSection } from './LocaleSection';
import { ShowAllViewsToggle } from './ShowAllViewsToggle';
import { AIProviderSection } from './AIProviderSection';
import { MonitoringSection } from './MonitoringSection';
import { DigestSection } from './DigestSection';
import { CommunityIntelligenceSection } from './CommunityIntelligenceSection';
import { LanguageSelector } from './LanguageSelector';
import { LicenseSection } from './LicenseSection';
import { StreetsMembershipSection } from './StreetsMembershipSection';

interface SettingsGeneralTabProps {
  settings: any;
  settingsForm: any;
  setSettingsForm: (form: any) => void;
  ollamaStatus: any;
  ollamaModels: any[];
  checkOllamaStatus: () => void;
  modelRegistry: any;
  onRefreshRegistry: () => void;
  monitoring: any;
  monitoringInterval: number;
  setMonitoringInterval: (v: number) => void;
  notificationThreshold: string;
  setNotificationThreshold: (v: string) => void;
  onToggleMonitoring: () => void;
  onUpdateInterval: () => void;
  onTestNotification: () => void;
  setSettingsStatus: (s: string) => void;
  saveSettings: () => void;
  testConnection: () => void;
}

export const SettingsGeneralTab = memo(function SettingsGeneralTab({
  settings,
  settingsForm,
  setSettingsForm,
  ollamaStatus,
  ollamaModels,
  checkOllamaStatus,
  modelRegistry,
  onRefreshRegistry,
  monitoring,
  monitoringInterval,
  setMonitoringInterval,
  notificationThreshold,
  setNotificationThreshold,
  onToggleMonitoring,
  onUpdateInterval,
  onTestNotification,
  setSettingsStatus,
  saveSettings,
  testConnection,
}: SettingsGeneralTabProps) {
  const { t } = useTranslation();

  return (
    <div id="tabpanel-general" role="tabpanel">
      <div className="space-y-6">
        <PanelErrorBoundary name="Language">
          <LocaleSection />
        </PanelErrorBoundary>

        <ShowAllViewsToggle />

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

        <PanelErrorBoundary name="Monitoring">
          <MonitoringSection
            monitoring={monitoring}
            monitoringInterval={monitoringInterval}
            setMonitoringInterval={setMonitoringInterval}
            notificationThreshold={notificationThreshold}
            onSetNotificationThreshold={setNotificationThreshold}
            onToggle={onToggleMonitoring}
            onUpdateInterval={onUpdateInterval}
            onTestNotification={onTestNotification}
          />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Digest">
          <DigestSection setSettingsStatus={setSettingsStatus} />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Community Intelligence">
          <CommunityIntelligenceSection />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Language">
          <LanguageSelector />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="License">
          <LicenseSection onStatus={setSettingsStatus} />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="STREETS Membership">
          <StreetsMembershipSection onStatus={setSettingsStatus} />
        </PanelErrorBoundary>

        {/* Actions */}
        <div className="flex gap-3 pt-2">
          <button
            onClick={saveSettings}
            aria-label={t('settings.saveSettings')}
            className="flex-1 px-4 py-3 text-sm bg-gradient-to-r from-orange-500 to-orange-600 text-white font-medium rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all shadow-lg shadow-orange-500/20"
          >
            {t('settings.saveSettings')}
          </button>
          <button
            onClick={testConnection}
            aria-label={t('settings.testConnection')}
            className="px-6 py-3 text-sm bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
          >
            {t('settings.testConnection')}
          </button>
        </div>
      </div>
    </div>
  );
});

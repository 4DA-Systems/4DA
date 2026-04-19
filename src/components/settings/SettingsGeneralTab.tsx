// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import type { MonitoringStatus } from '../../types/settings';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { LocaleSection } from './LocaleSection';
import { ShowAllViewsToggle } from './ShowAllViewsToggle';
import { MonitoringSection } from './MonitoringSection';
import { DataHealthSection } from './DataHealthSection';
import { SystemHealthSection } from './SystemHealthSection';
import { DigestSection } from './DigestSection';
import { CommunityIntelligenceSection } from './CommunityIntelligenceSection';
import { PrivacySection } from './PrivacySection';

interface SettingsGeneralTabProps {
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (v: number) => void;
  notificationThreshold: string;
  setNotificationThreshold: (v: string) => void;
  onToggleMonitoring: () => void;
  onUpdateInterval: () => void;
  onTestNotification: () => void;
  setSettingsStatus: (s: string) => void;
}

export const SettingsGeneralTab = memo(function SettingsGeneralTab({
  monitoring,
  monitoringInterval,
  setMonitoringInterval,
  notificationThreshold,
  setNotificationThreshold,
  onToggleMonitoring,
  onUpdateInterval,
  onTestNotification,
  setSettingsStatus,
}: SettingsGeneralTabProps) {
  return (
    <div id="tabpanel-general" role="tabpanel">
      <div className="space-y-4">
        <PanelErrorBoundary name="Language">
          <LocaleSection />
        </PanelErrorBoundary>

        <ShowAllViewsToggle />

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

        <PanelErrorBoundary name="System Health">
          <SystemHealthSection />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Data Health">
          <DataHealthSection />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Digest">
          <DigestSection setSettingsStatus={setSettingsStatus} />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Community Intelligence">
          <CommunityIntelligenceSection />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Privacy">
          <PrivacySection />
        </PanelErrorBoundary>
      </div>
    </div>
  );
});

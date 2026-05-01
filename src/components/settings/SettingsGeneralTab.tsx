// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { MonitoringStatus } from '../../types/settings';
import { PanelErrorBoundary } from '../PanelErrorBoundary';
import { LocaleSection } from './LocaleSection';
import { MonitoringSection } from './MonitoringSection';
import { cmd } from '../../lib/commands';

interface SettingsGeneralTabProps {
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (v: number) => void;
  onToggleMonitoring: () => void;
  onUpdateInterval: () => void;
}

export const SettingsGeneralTab = memo(function SettingsGeneralTab({
  monitoring,
  monitoringInterval,
  setMonitoringInterval,
  onToggleMonitoring,
  onUpdateInterval,
}: SettingsGeneralTabProps) {
  const { t } = useTranslation();
  const [retentionDays, setRetentionDays] = useState(30);
  const [retentionSaving, setRetentionSaving] = useState(false);

  useEffect(() => {
    cmd('get_data_health').then((data) => {
      setRetentionDays(data.retention_days);
    }).catch(() => {});
  }, []);

  const handleRetentionChange = useCallback(async (days: number) => {
    setRetentionDays(days);
    setRetentionSaving(true);
    try {
      await cmd('set_cleanup_retention', { days });
    } catch {
      // Revert handled by next load
    } finally {
      setRetentionSaving(false);
    }
  }, []);

  return (
    <div id="tabpanel-general" role="tabpanel">
      <div className="space-y-4">
        <PanelErrorBoundary name="Language">
          <LocaleSection />
        </PanelErrorBoundary>

        <PanelErrorBoundary name="Monitoring">
          <MonitoringSection
            monitoring={monitoring}
            monitoringInterval={monitoringInterval}
            setMonitoringInterval={setMonitoringInterval}
            onToggle={onToggleMonitoring}
            onUpdateInterval={onUpdateInterval}
          />
        </PanelErrorBoundary>

        <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-medium text-white">
              {t('settings.dataHealth.retention')}
            </span>
            <span className="text-xs text-text-secondary font-mono">
              {retentionDays} {t('settings.dataHealth.days')}
              {retentionSaving ? <span className="text-orange-400 ms-1">{t('settings.dataHealth.saving')}</span> : null}
            </span>
          </div>
          <input
            type="range"
            min={7}
            max={365}
            step={1}
            value={retentionDays}
            onChange={(e) => { void handleRetentionChange(parseInt(e.target.value)); }}
            className="w-full h-1 bg-border rounded-full appearance-none cursor-pointer accent-orange-500"
          />
          <div className="flex justify-between text-[10px] text-text-muted mt-1">
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span>7d</span><span>30d</span><span>90d</span><span>180d</span><span>365d</span>
          </div>
        </div>
      </div>
    </div>
  );
});

import { cmd } from '../../lib/commands';
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { MonitoringStatus } from '../../types';

interface MonitoringSectionProps {
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (val: number) => void;
  notificationThreshold: string;
  onSetNotificationThreshold: (threshold: string) => void;
  onToggle: () => void;
  onUpdateInterval: () => void;
  onTestNotification: () => void;
}

function CloseToTrayToggle({ initialValue }: { initialValue: boolean }) {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(initialValue);

  const toggle = async () => {
    const next = !enabled;
    setEnabled(next);
    try {
      await cmd('set_close_to_tray', { enabled: next });
    } catch {
      setEnabled(!next); // revert on error
    }
  };

  return (
    <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
      <div>
        <span className="text-sm text-white">{t('settings.monitoring.closeToTray')}</span>
        <p className="text-xs text-text-muted">{t('settings.monitoring.closeToTrayDescription')}</p>
      </div>
      <button
        onClick={toggle}
        className={`relative w-10 h-5 rounded-full transition-colors ${
          enabled ? 'bg-green-500/40' : 'bg-gray-600'
        }`}
      >
        <span
          className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
            enabled ? 'translate-x-5' : 'translate-x-0.5'
          }`}
        />
      </button>
    </div>
  );
}

function LaunchAtStartupToggle() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(false);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    cmd('get_launch_at_startup').then(v => { setEnabled(v); setLoaded(true); }).catch(() => setLoaded(true));
  }, []);

  const toggle = async () => {
    const next = !enabled;
    setEnabled(next);
    try {
      await cmd('set_launch_at_startup', { enabled: next });
    } catch {
      setEnabled(!next);
    }
  };

  if (!loaded) return null;

  return (
    <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
      <div>
        <span className="text-sm text-white">{t('settings.monitoring.launchAtStartup', 'Launch at startup')}</span>
        <p className="text-xs text-text-muted">{t('settings.monitoring.launchAtStartupDescription', 'Start 4DA automatically when you log in')}</p>
      </div>
      <button
        onClick={toggle}
        className={`relative w-10 h-5 rounded-full transition-colors ${
          enabled ? 'bg-green-500/40' : 'bg-gray-600'
        }`}
      >
        <span
          className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
            enabled ? 'translate-x-5' : 'translate-x-0.5'
          }`}
        />
      </button>
    </div>
  );
}

function MorningBriefingSection() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(true);
  const [time, setTime] = useState('08:00');
  const [loaded, setLoaded] = useState(false);
  const [previewing, setPreviewing] = useState(false);

  useEffect(() => {
    cmd('get_morning_briefing_config')
      .then((config) => {
        setEnabled(config.enabled);
        setTime(config.time);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const toggleBriefing = async () => {
    const next = !enabled;
    setEnabled(next);
    try {
      await cmd('set_morning_briefing_enabled', { enabled: next });
    } catch {
      setEnabled(!next);
    }
  };

  const updateTime = async (newTime: string) => {
    setTime(newTime);
    try {
      await cmd('set_briefing_time', { time: newTime });
    } catch {
      // Revert handled by next load
    }
  };

  const previewBriefing = async () => {
    setPreviewing(true);
    try {
      await cmd('trigger_briefing_preview');
    } catch {
      // Non-fatal
    }
    setTimeout(() => setPreviewing(false), 2000);
  };

  if (!loaded) return null;

  return (
    <div className="space-y-3 p-3 bg-bg-secondary rounded-lg border border-border">
      <div className="flex items-center justify-between">
        <div>
          <span className="text-sm text-white font-medium">
            {t('settings.monitoring.morningBriefing', 'Intelligence Briefing')}
          </span>
          <p className="text-xs text-text-muted">
            {t('settings.monitoring.morningBriefingDescription', 'Center-screen daily briefing with signals, knowledge gaps, and escalating chains')}
          </p>
        </div>
        <button
          onClick={toggleBriefing}
          className={`relative w-10 h-5 rounded-full transition-colors ${
            enabled ? 'bg-green-500/40' : 'bg-gray-600'
          }`}
        >
          <span
            className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
              enabled ? 'translate-x-5' : 'translate-x-0.5'
            }`}
          />
        </button>
      </div>

      {enabled && (
        <div className="flex items-center gap-3">
          <label className="text-xs text-text-secondary">
            {t('settings.monitoring.briefingTime', 'Briefing time')}
          </label>
          <input
            type="time"
            value={time}
            onChange={(e) => updateTime(e.target.value)}
            className="px-2 py-1 bg-bg-primary border border-border rounded text-sm text-white focus:border-orange-500 focus:outline-none"
          />
          <button
            onClick={previewBriefing}
            disabled={previewing}
            className="ms-auto px-3 py-1 text-xs bg-bg-primary border border-border text-text-secondary rounded hover:text-white hover:border-orange-500/30 transition-all disabled:opacity-50"
          >
            {previewing
              ? t('settings.monitoring.briefingPreviewing', 'Showing...')
              : t('settings.monitoring.briefingPreview', 'Preview briefing')}
          </button>
        </div>
      )}
    </div>
  );
}

export function MonitoringSection({
  monitoring,
  monitoringInterval,
  setMonitoringInterval,
  notificationThreshold,
  onSetNotificationThreshold,
  onToggle,
  onUpdateInterval,
  onTestNotification,
}: MonitoringSectionProps) {
  const { t } = useTranslation();
  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center gap-3 mb-3">
        <div className="w-8 h-8 bg-blue-500/20 rounded-lg flex items-center justify-center">
          <span>&#x1f504;</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-white">{t('settings.monitoring.backgroundTitle')}</h3>
          <p className="text-xs text-text-muted">{t('settings.monitoring.backgroundDescription')}</p>
        </div>
      </div>

      {monitoring ? (
        <div className="space-y-3">
          <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="flex items-center gap-2">
              {monitoring.enabled ? (
                <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              ) : (
                <div className="w-2 h-2 bg-gray-600 rounded-full" />
              )}
              <span className="text-sm text-white">
                {monitoring.enabled ? (
                  <span className="text-green-400">{t('status.active')}</span>
                ) : (
                  <span className="text-text-muted">{t('status.inactive')}</span>
                )}
              </span>
              {monitoring.is_checking && (
                <span className="text-xs text-orange-400 ms-2">({t('settings.monitoring.checking')})</span>
              )}
            </div>
            <button
              onClick={onToggle}
              className={`px-4 py-2 text-sm rounded-lg transition-all ${
                monitoring.enabled
                  ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
                  : 'bg-green-500/20 text-green-400 hover:bg-green-500/30'
              }`}
            >
              {monitoring.enabled ? t('settings.monitoring.stop') : t('settings.monitoring.start')}
            </button>
          </div>

          {/* Schedule & Notifications */}
          <div className="border-t border-border/50 pt-3" />
          <div className="flex items-center gap-3">
            <label className="text-sm text-text-secondary">{t('settings.monitoring.every')}</label>
            <input
              type="number"
              min="5"
              max="1440"
              value={monitoringInterval}
              onChange={(e) => setMonitoringInterval(parseInt(e.target.value) || 30)}
              className="w-20 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white text-center focus:border-orange-500 focus:outline-none"
            />
            <span className="text-sm text-text-secondary">{t('settings.monitoring.minutes')}</span>
            <span className="text-xs text-text-muted">5 min – 24 hr</span>
            <button
              onClick={onUpdateInterval}
              className="px-4 py-2 text-sm bg-bg-secondary border border-border text-text-secondary rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
            >
              {t('settings.monitoring.update')}
            </button>
          </div>

          <div className="flex items-center gap-3">
            <label className="text-sm text-text-secondary">{t('settings.monitoring.notifications')}</label>
            <select
              value={notificationThreshold}
              onChange={(e) => onSetNotificationThreshold(e.target.value)}
              className="px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none appearance-none cursor-pointer"
              style={{ backgroundImage: 'url("data:image/svg+xml,%3Csvg xmlns=\'http://www.w3.org/2000/svg\' width=\'12\' height=\'12\' fill=\'%23666\' viewBox=\'0 0 16 16\'%3E%3Cpath d=\'M8 11L3 6h10z\'/%3E%3C/svg%3E")', backgroundRepeat: 'no-repeat', backgroundPosition: 'right 0.75rem center', paddingRight: '2rem' }}
            >
              <option value="critical_only">{t('settings.monitoring.criticalOnly', 'Critical only')}</option>
              <option value="high_and_above">{t('settings.monitoring.highAndAbove', 'High and above')}</option>
              <option value="all">{t('settings.monitoring.allItems')}</option>
            </select>
            <p className="text-xs text-text-muted mt-1">Critical = security advisories, High = important signals</p>
          </div>

          <div className="flex items-center gap-3">
            <label className="text-sm text-text-secondary">{t('settings.monitoring.notificationStyle', 'Notification style')}</label>
            <select
              value={monitoring.notification_style || 'custom'}
              onChange={(e) => {
                cmd('set_notification_style', { style: e.target.value }).catch((err) => console.debug('[MonitoringSection] notification style:', err));
              }}
              className="px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white focus:border-orange-500 focus:outline-none appearance-none cursor-pointer"
              style={{ backgroundImage: 'url("data:image/svg+xml,%3Csvg xmlns=\'http://www.w3.org/2000/svg\' width=\'12\' height=\'12\' fill=\'%23666\' viewBox=\'0 0 16 16\'%3E%3Cpath d=\'M8 11L3 6h10z\'/%3E%3C/svg%3E")', backgroundRepeat: 'no-repeat', backgroundPosition: 'right 0.75rem center', paddingRight: '2rem' }}
            >
              <option value="custom">{t('settings.monitoring.styleCustom', 'Glyph atmosphere')}</option>
              <option value="native">{t('settings.monitoring.styleNative', 'OS native')}</option>
            </select>
          </div>

          {/* Behavior */}
          <div className="border-t border-border/50 pt-3" />
          <MorningBriefingSection />

          <CloseToTrayToggle initialValue={monitoring.close_to_tray} />
          <LaunchAtStartupToggle />

          {/* Status — only show when monitoring has actually run */}
          {monitoring.total_checks > 0 && (
            <>
              <div className="border-t border-border/50 pt-3" />
              <div className="flex items-center justify-between text-xs text-text-muted px-1">
                {monitoring.last_check_ago && (
                  <span>{t('settings.monitoring.lastCheck', { time: monitoring.last_check_ago })}</span>
                )}
              </div>
            </>
          )}

          <button
            onClick={onTestNotification}
            className="w-full px-4 py-2.5 text-sm bg-bg-secondary border border-border text-text-secondary rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
          >
            {t('settings.monitoring.testNotification')}
          </button>
        </div>
      ) : (
        <div className="text-xs text-text-muted">{t('settings.monitoring.loading')}</div>
      )}
    </div>
  );
}

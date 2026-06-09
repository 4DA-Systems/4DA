// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { cmd, type SchedulerStatus } from '../../lib/commands';
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { MonitoringStatus } from '../../types';

interface MonitoringSectionProps {
  monitoring: MonitoringStatus | null;
  monitoringInterval: number;
  setMonitoringInterval: (val: number) => void;
  onToggle: () => void;
  onUpdateInterval: () => void;
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
        onClick={() => { void toggle(); }}
        className={`relative w-10 h-5 rounded-full transition-colors ${
          enabled ? 'bg-green-500/40' : 'bg-gray-600'
        }`}
      >
        <span
          className={`absolute left-0 top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
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
  const [regFailed, setRegFailed] = useState(false);

  useEffect(() => {
    cmd('get_launch_at_startup').then(v => { setEnabled(v); setLoaded(true); }).catch(() => setLoaded(true));
  }, []);

  const toggle = async () => {
    const next = !enabled;
    setEnabled(next);
    setRegFailed(false);
    try {
      const result = await cmd('set_launch_at_startup', { enabled: next });
      // Trust the backend's actual state, not our optimistic update
      setEnabled(result.launch_at_startup);
      if (result.registration_failed) {
        setRegFailed(true);
      }
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
        {regFailed && (
          <p className="text-xs text-red-400 mt-1">
            {t('settings.monitoring.autostartFailed', 'Could not register with the operating system. Check permissions or desktop environment support.')}
          </p>
        )}
      </div>
      <button
        onClick={() => { void toggle(); }}
        className={`relative w-10 h-5 rounded-full transition-colors ${
          enabled ? 'bg-green-500/40' : 'bg-gray-600'
        }`}
      >
        <span
          className={`absolute left-0 top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
            enabled ? 'translate-x-5' : 'translate-x-0.5'
          }`}
        />
      </button>
    </div>
  );
}

/**
 * Toggles an OS scheduled task that refreshes the feed while 4DA is closed — the counterpart to the
 * in-app monitoring above (which only runs while the window/tray is alive). Backed by the
 * install/uninstall/status background-refresh Tauri commands; the OS task is the source of truth, so
 * we always render the backend's reported state rather than an optimistic guess.
 */
function BackgroundRefreshToggle({ intervalMinutes }: { intervalMinutes: number }) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<SchedulerStatus | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    cmd('background_refresh_status')
      .then((s) => { setStatus(s); setLoaded(true); })
      .catch(() => setLoaded(true));
  }, []);

  const installed = status?.installed ?? false;
  const supported = status?.supported ?? true;
  // Prefer the installed task's interval; else the user's monitoring interval; else the scheduler
  // default (30) — never blank, so "every {{minutes}} min" always renders a number.
  const activeInterval = status?.interval_minutes ?? (intervalMinutes > 0 ? intervalMinutes : 30);

  // Keep the installed task's cadence in sync with the user's monitoring interval: if enabled and the
  // active interval no longer matches, re-install (schtasks /F overwrites in place). Debounced so
  // typing in the interval field doesn't churn the task; ignores values below the field's 5-min min.
  useEffect(() => {
    if (!installed || !supported || intervalMinutes < 5) return;
    if (status?.interval_minutes == null || status.interval_minutes === intervalMinutes) return;
    const timer = setTimeout(() => {
      cmd('install_background_refresh', { intervalMinutes })
        .then((s) => setStatus(s))
        .catch(() => {});
    }, 1500);
    return () => clearTimeout(timer);
  }, [installed, supported, intervalMinutes, status?.interval_minutes]);

  const toggle = async () => {
    if (busy || !supported) return;
    setBusy(true);
    setFailed(false);
    try {
      const next = installed
        ? await cmd('uninstall_background_refresh')
        : await cmd('install_background_refresh', { intervalMinutes });
      setStatus(next);
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  if (!loaded) return null;

  return (
    <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
      <div>
        <span className="text-sm text-white">{t('settings.monitoring.backgroundRefresh', 'Refresh while 4DA is closed')}</span>
        <p className="text-xs text-text-muted">
          {!supported
            ? t('settings.monitoring.backgroundRefreshUnsupported', 'Not available on this platform yet.')
            : installed
              ? t('settings.monitoring.backgroundRefreshActive', { minutes: activeInterval })
              : t('settings.monitoring.backgroundRefreshDescription', "Run a background system task that keeps your feed fresh on your monitoring interval, even when 4DA isn't open.")}
        </p>
        {failed && (
          <p className="text-xs text-red-400 mt-1">
            {t('settings.monitoring.backgroundRefreshFailed', 'Could not update the system task. Check your permissions and try again.')}
          </p>
        )}
        {supported && (
          <button
            type="button"
            onClick={() => setExpanded((e) => !e)}
            aria-expanded={expanded}
            className="mt-1.5 inline-flex items-center gap-1 text-xs text-text-secondary hover:text-white transition-colors"
          >
            {t('settings.monitoring.backgroundRefreshWhat', 'What this does')}
            <svg
              className={`w-3 h-3 transition-transform ${expanded ? 'rotate-180' : ''}`}
              viewBox="0 0 12 12"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              aria-hidden="true"
            >
              <path d="M3 4.5 6 7.5 9 4.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        )}
        {expanded && supported && (
          <p className="text-xs text-text-muted mt-1.5 leading-relaxed max-w-md">
            {t('settings.monitoring.backgroundRefreshWhatDetail', { minutes: activeInterval })}
          </p>
        )}
      </div>
      <button
        onClick={() => { void toggle(); }}
        disabled={busy || !supported}
        aria-pressed={installed}
        aria-label={t('settings.monitoring.backgroundRefresh', 'Refresh while 4DA is closed')}
        className={`relative w-10 h-5 rounded-full transition-colors ${
          installed ? 'bg-green-500/40' : 'bg-gray-600'
        } ${busy || !supported ? 'opacity-50 cursor-not-allowed' : ''}`}
      >
        <span
          className={`absolute left-0 top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
            installed ? 'translate-x-5' : 'translate-x-0.5'
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
          onClick={() => { void toggleBriefing(); }}
          className={`relative w-10 h-5 rounded-full transition-colors ${
            enabled ? 'bg-green-500/40' : 'bg-gray-600'
          }`}
        >
          <span
            className={`absolute left-0 top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
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
            onChange={(e) => { void updateTime(e.target.value); }}
            className="px-2 py-1 bg-bg-primary border border-border rounded text-sm text-white focus:border-orange-500 focus:outline-none"
          />
          <button
            onClick={() => { void previewBriefing(); }}
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
  onToggle,
  onUpdateInterval,
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
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-xs text-text-muted">5 min – 24 hr</span>
            <button
              onClick={onUpdateInterval}
              className="px-4 py-2 text-sm bg-bg-secondary border border-border text-text-secondary rounded-lg hover:text-white hover:border-orange-500/30 transition-all"
            >
              {t('settings.monitoring.update')}
            </button>
          </div>

          {/* Behavior */}
          <div className="border-t border-border/50 pt-3" />
          <MorningBriefingSection />

          <CloseToTrayToggle initialValue={monitoring.close_to_tray} />
          <LaunchAtStartupToggle />
          <BackgroundRefreshToggle intervalMinutes={monitoringInterval} />

        </div>
      ) : (
        <div className="text-xs text-text-muted">{t('settings.monitoring.loading')}</div>
      )}
    </div>
  );
}

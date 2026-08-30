// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd, type InterruptionConfig, type PresenceStatus } from '../../lib/commands';

/** How often the live status line re-polls while the settings panel is open. */
const STATUS_POLL_MS = 5000;

/** Do Not Disturb durations offered as one-tap chips, in minutes. */
const DND_DURATIONS = [30, 60, 120] as const;

function Toggle({
  enabled,
  onClick,
  label,
}: {
  enabled: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      onClick={onClick}
      aria-pressed={enabled}
      aria-label={label}
      className={`relative w-10 h-5 rounded-full transition-colors shrink-0 ${
        enabled ? 'bg-green-500/40' : 'bg-gray-600'
      }`}
    >
      <span
        className={`absolute left-0 top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
          enabled ? 'translate-x-5' : 'translate-x-0.5'
        }`}
      />
    </button>
  );
}

export function InterruptionsSection() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<InterruptionConfig | null>(null);
  const [status, setStatus] = useState<PresenceStatus | null>(null);

  const refreshStatus = useCallback(() => {
    cmd('get_presence_status')
      .then(setStatus)
      .catch(() => {
        /* status is advisory — a failed poll must not break the panel */
      });
  }, []);

  useEffect(() => {
    cmd('get_interruption_config')
      .then(setConfig)
      .catch(() => setConfig(null));
    refreshStatus();
    const timer = setInterval(refreshStatus, STATUS_POLL_MS);
    return () => clearInterval(timer);
  }, [refreshStatus]);

  const toggleRespectFocus = async () => {
    if (!config) return;
    const next = !config.respect_focus;
    setConfig({ ...config, respect_focus: next });
    try {
      await cmd('set_respect_focus', { enabled: next });
      refreshStatus();
    } catch {
      setConfig({ ...config, respect_focus: !next });
    }
  };

  const setDnd = async (enabled: boolean, minutes: number | null) => {
    if (!config) return;
    const previous = config;
    setConfig({ ...config, dnd_active: enabled });
    try {
      const next = await cmd('set_do_not_disturb', { enabled, minutes });
      setStatus(next);
      const fresh = await cmd('get_interruption_config');
      setConfig(fresh);
    } catch {
      setConfig(previous);
    }
  };

  const updateQuietHours = async (start: string | null, end: string | null) => {
    if (!config) return;
    const previous = config;
    setConfig({ ...config, quiet_hours_start: start, quiet_hours_end: end });
    try {
      await cmd('set_quiet_hours', { start, end });
      refreshStatus();
    } catch {
      setConfig(previous);
    }
  };

  const showHeldNow = async () => {
    try {
      await cmd('flush_held_notifications');
      refreshStatus();
    } catch {
      /* non-fatal */
    }
  };

  const dismissHeld = async () => {
    try {
      await cmd('discard_held_notifications');
      refreshStatus();
    } catch {
      /* non-fatal */
    }
  };

  if (!config) return null;

  const quietHoursOn = Boolean(config.quiet_hours_start && config.quiet_hours_end);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center gap-3 mb-3">
        <div className="w-8 h-8 bg-purple-500/20 rounded-lg flex items-center justify-center">
          <span>&#x1f507;</span>
        </div>
        <div>
          <h3 className="text-sm font-medium text-text-primary">
            {t('settings.interruptions.title', 'Interruptions')}
          </h3>
          <p className="text-xs text-text-muted">
            {t(
              'settings.interruptions.description',
              'Nothing is dropped. What arrives while you are busy is held and delivered when you are free.',
            )}
          </p>
        </div>
      </div>

      <div className="space-y-3">
        {/* Live status */}
        {status && (
          <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
            <div className="flex items-center gap-2 min-w-0">
              <div
                className={`w-2 h-2 rounded-full shrink-0 ${
                  status.available ? 'bg-green-500' : 'bg-amber-500'
                }`}
              />
              <span className="text-sm text-text-primary truncate">
                {status.available
                  ? t('settings.interruptions.statusAvailable', 'Ready to notify you')
                  : t('settings.interruptions.statusHolding', 'Holding {{reason}}', {
                      reason: status.reason_text ?? '',
                    })}
              </span>
            </div>
            {status.held_count > 0 && (
              <div className="flex items-center gap-2 shrink-0">
                <span className="text-xs text-amber-400">
                  {t('settings.interruptions.heldCount', '{{count}} waiting', {
                    count: status.held_count,
                  })}
                </span>
                <button
                  onClick={() => { void showHeldNow(); }}
                  className="px-2 py-1 text-xs bg-bg-primary border border-border text-text-secondary rounded hover:text-text-primary hover:border-orange-500/30 transition-all"
                >
                  {t('settings.interruptions.showNow', 'Show now')}
                </button>
                <button
                  onClick={() => { void dismissHeld(); }}
                  className="px-2 py-1 text-xs bg-bg-primary border border-border text-text-muted rounded hover:text-text-primary transition-all"
                >
                  {t('settings.interruptions.dismiss', 'Dismiss')}
                </button>
              </div>
            )}
          </div>
        )}

        {/* Respect fullscreen & focus */}
        <div className="flex items-center justify-between p-3 bg-bg-secondary rounded-lg border border-border">
          <div className="pe-3">
            <span className="text-sm text-text-primary">
              {t('settings.interruptions.respectFocus', 'Respect fullscreen and focus')}
            </span>
            <p className="text-xs text-text-muted">
              {status?.os_detection_supported === false
                ? t(
                    'settings.interruptions.respectFocusUnsupported',
                    'Fullscreen detection is not available on this platform yet — quiet hours and Do Not Disturb still apply.',
                  )
                : t(
                    'settings.interruptions.respectFocusDescription',
                    'Stay quiet during games, fullscreen apps, presentations, and while Focus Assist is on.',
                  )}
            </p>
          </div>
          <Toggle
            enabled={config.respect_focus}
            onClick={() => { void toggleRespectFocus(); }}
            label={t('settings.interruptions.respectFocus', 'Respect fullscreen and focus')}
          />
        </div>

        {/* Do Not Disturb */}
        <div className="p-3 bg-bg-secondary rounded-lg border border-border space-y-2">
          <div className="flex items-center justify-between">
            <div className="pe-3">
              <span className="text-sm text-text-primary">
                {t('settings.interruptions.dnd', 'Do Not Disturb')}
              </span>
              <p className="text-xs text-text-muted">
                {config.dnd_active && config.dnd_until
                  ? t('settings.interruptions.dndUntil', 'On until {{time}}', {
                      time: new Date(config.dnd_until).toLocaleTimeString([], {
                        hour: '2-digit',
                        minute: '2-digit',
                      }),
                    })
                  : t(
                      'settings.interruptions.dndDescription',
                      'Hold everything until you turn it off.',
                    )}
              </p>
            </div>
            <Toggle
              enabled={config.dnd_active}
              onClick={() => { void setDnd(!config.dnd_active, null); }}
              label={t('settings.interruptions.dnd', 'Do Not Disturb')}
            />
          </div>
          {!config.dnd_active && (
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary">
                {t('settings.interruptions.dndFor', 'Pause for')}
              </span>
              {DND_DURATIONS.map((minutes) => (
                <button
                  key={minutes}
                  onClick={() => { void setDnd(true, minutes); }}
                  className="px-2 py-1 text-xs bg-bg-primary border border-border text-text-secondary rounded hover:text-text-primary hover:border-orange-500/30 transition-all"
                >
                  {minutes < 60
                    ? t('settings.interruptions.durationMinutes', '{{count}}m', { count: minutes })
                    : t('settings.interruptions.durationHours', '{{count}}h', {
                        count: minutes / 60,
                      })}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Quiet hours */}
        <div className="p-3 bg-bg-secondary rounded-lg border border-border space-y-2">
          <div className="flex items-center justify-between">
            <div className="pe-3">
              <span className="text-sm text-text-primary">
                {t('settings.interruptions.quietHours', 'Quiet hours')}
              </span>
              <p className="text-xs text-text-muted">
                {t(
                  'settings.interruptions.quietHoursDescription',
                  'A window that repeats daily. It may cross midnight.',
                )}
              </p>
            </div>
            <Toggle
              enabled={quietHoursOn}
              onClick={() => {
                void (quietHoursOn
                  ? updateQuietHours(null, null)
                  : updateQuietHours('22:00', '07:00'));
              }}
              label={t('settings.interruptions.quietHours', 'Quiet hours')}
            />
          </div>
          {quietHoursOn && (
            <div className="flex items-center gap-3">
              <label className="text-xs text-text-secondary">
                {t('settings.interruptions.from', 'From')}
              </label>
              <input
                type="time"
                value={config.quiet_hours_start ?? ''}
                onChange={(e) => {
                  void updateQuietHours(e.target.value, config.quiet_hours_end);
                }}
                className="px-2 py-1 bg-bg-primary border border-border rounded text-sm text-text-primary focus:border-orange-500 focus:outline-none"
              />
              <label className="text-xs text-text-secondary">
                {t('settings.interruptions.to', 'to')}
              </label>
              <input
                type="time"
                value={config.quiet_hours_end ?? ''}
                onChange={(e) => {
                  void updateQuietHours(config.quiet_hours_start, e.target.value);
                }}
                className="px-2 py-1 bg-bg-primary border border-border rounded text-sm text-text-primary focus:border-orange-500 focus:outline-none"
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

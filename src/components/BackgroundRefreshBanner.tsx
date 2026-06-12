// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../lib/commands';
import { useAppStore } from '../store';

/**
 * One-time, dismissible nudge that makes background refresh discoverable. The toggle that enables it
 * lives in Settings → Monitoring and is off by default, so the users who most need it — the ones who
 * close 4DA and find stale data — would never find it. This surfaces it once, at app level, with a
 * single-click Enable.
 *
 * Shown only when: the user is past first-run, the platform supports scheduling, the task is not
 * already installed, and they haven't enabled or dismissed it before (persisted in localStorage so it
 * does not return on the next launch). Enabling installs the OS task at the scheduler default (30 min);
 * the cadence then follows the monitoring interval (see BackgroundRefreshToggle).
 */
const DISMISS_KEY = '4da-bgrefresh-suggest-dismissed';

export function BackgroundRefreshBanner() {
  const { t } = useTranslation();
  const isFirstRun = useAppStore((s) => s.isFirstRun);
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    if (isFirstRun) return;
    if (localStorage.getItem(DISMISS_KEY)) return;
    let cancelled = false;
    cmd('background_refresh_status')
      .then((s) => {
        if (!cancelled && s.supported && !s.installed) setShow(true);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [isFirstRun]);

  if (!show) return null;

  const dismiss = () => {
    localStorage.setItem(DISMISS_KEY, '1');
    setShow(false);
  };

  const enable = async () => {
    setBusy(true);
    try {
      const status = await cmd('install_background_refresh', {});
      if (status.installed) {
        localStorage.setItem(DISMISS_KEY, '1');
        setEnabled(true);
        // Show the confirmation briefly, then retire the banner.
        setTimeout(() => setShow(false), 2400);
      }
    } catch {
      // Leave the banner up so the user can retry or dismiss.
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-4 mt-2 mb-1 bg-blue-500/8 border border-blue-500/25 rounded-lg overflow-hidden">
      <div className="px-3 py-2 flex items-center justify-between gap-3">
        <div className="min-w-0">
          {enabled ? (
            <span className="text-sm text-text-primary">
              {t(
                'settings.monitoring.backgroundRefreshNudgeEnabled',
                'Background refresh is on — your feed stays fresh even when 4DA is closed.',
              )}
            </span>
          ) : (
            <>
              <span className="text-sm text-text-primary">
                {t('settings.monitoring.backgroundRefreshNudgeTitle', 'Keep your feed fresh when 4DA is closed')}
              </span>
              <p className="text-xs text-text-muted">
                {t(
                  'settings.monitoring.backgroundRefreshNudgeBody',
                  "4DA only gathers signals while it's open. Turn on background refresh and it keeps your feed current even when the app is closed.",
                )}
              </p>
            </>
          )}
        </div>
        {!enabled && (
          <div className="flex items-center gap-2 shrink-0">
            <button
              onClick={() => {
                void enable();
              }}
              disabled={busy}
              className="px-3 py-1.5 text-xs rounded bg-blue-500/25 text-text-primary hover:bg-blue-500/35 transition-colors disabled:opacity-50 whitespace-nowrap"
            >
              {busy
                ? t('settings.monitoring.backgroundRefreshNudgeEnabling', 'Enabling…')
                : t('settings.monitoring.backgroundRefreshNudgeEnable', 'Enable')}
            </button>
            <button
              onClick={dismiss}
              className="px-3 py-1.5 text-xs rounded bg-text-primary/5 text-text-secondary hover:text-text-primary hover:bg-text-primary/10 transition-colors whitespace-nowrap"
            >
              {t('settings.monitoring.backgroundRefreshNudgeDismiss', 'Not now')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

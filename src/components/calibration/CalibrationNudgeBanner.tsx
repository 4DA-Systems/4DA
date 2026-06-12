// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';
import { CalibrationView } from './CalibrationView';

/**
 * One-time, dismissible nudge toward the calibration surface — modeled
 * on BackgroundRefreshBanner. The taste test shipped inside onboarding,
 * so every install that predates it (and every user who skipped it) has
 * an uncalibrated instance with no way back in. This surfaces the door
 * once, at app level.
 *
 * Shown only when: past first-run, not previously dismissed (localStorage),
 * AND the instance actually needs calibrating — taste test never taken OR
 * fewer than 10 explicit labels recorded. Never shown during onboarding.
 */
const DISMISS_KEY = '4da-calibration-nudge-dismissed';

/** Below this many explicit labels the nudge still has a real job. */
const NUDGE_LABEL_FLOOR = 10;

export function CalibrationNudgeBanner() {
  const { t } = useTranslation();
  const isFirstRun = useAppStore((s) => s.isFirstRun);
  const [show, setShow] = useState(false);
  const [viewOpen, setViewOpen] = useState(false);

  useEffect(() => {
    if (isFirstRun) return;
    if (localStorage.getItem(DISMISS_KEY)) return;
    let cancelled = false;
    Promise.all([cmd('taste_test_is_calibrated'), cmd('get_calibration_sprint_status')])
      .then(([calibrated, status]) => {
        if (cancelled) return;
        if (!calibrated || status.labeledTotal < NUDGE_LABEL_FLOOR) setShow(true);
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

  const openCalibration = () => {
    // Opening counts as engaging — the banner has done its one-time job.
    localStorage.setItem(DISMISS_KEY, '1');
    setViewOpen(true);
  };

  return (
    <>
      <div className="mx-4 mt-2 mb-1 bg-accent-gold/8 border border-accent-gold/25 rounded-lg overflow-hidden">
        <div className="px-3 py-2 flex items-center justify-between gap-3">
          <div className="min-w-0">
            <span className="text-sm text-text-primary">{t('calibrationView.nudge.title')}</span>
            <p className="text-xs text-text-muted">{t('calibrationView.nudge.body')}</p>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <button
              onClick={openCalibration}
              className="px-3 py-1.5 text-xs rounded bg-accent-gold/25 text-text-primary hover:bg-accent-gold/35 transition-colors whitespace-nowrap"
            >
              {t('calibrationView.nudge.action')}
            </button>
            <button
              onClick={dismiss}
              className="px-3 py-1.5 text-xs rounded bg-text-primary/5 text-text-secondary hover:text-text-primary hover:bg-text-primary/10 transition-colors whitespace-nowrap"
            >
              {t('calibrationView.nudge.dismiss')}
            </button>
          </div>
        </div>
      </div>
      <CalibrationView
        open={viewOpen}
        onClose={() => {
          setViewOpen(false);
          setShow(false);
        }}
      />
    </>
  );
}

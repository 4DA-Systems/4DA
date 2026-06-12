// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { CalibrationSprintStatus } from '../../../src-tauri/bindings/bindings/CalibrationSprintStatus';
import { CalibrationView } from './CalibrationView';

/**
 * Settings affordance for the calibration surface — a sibling of
 * BriefNarrationStatus in the AI/Intelligence section. Shows the honest
 * fit progress (distinct labels recorded vs the fitter's real floor)
 * and opens the CalibrationView overlay. FREE tier — calibration is the
 * product promise, never gated.
 */
export function CalibrationSettingsRow() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<CalibrationSprintStatus | null>(null);
  const [open, setOpen] = useState(false);

  const refresh = () => {
    cmd('get_calibration_sprint_status').then(setStatus).catch(() => {});
  };

  useEffect(refresh, []);

  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-4 flex items-center justify-between gap-3">
      <div className="min-w-0">
        <p className="text-sm text-text-primary">{t('calibrationView.settingsRow.title')}</p>
        <p className="text-xs text-text-muted">{t('calibrationView.settingsRow.description')}</p>
        {status &&
          (status.curveFitted ? (
            <p className="text-xs text-success mt-1">{t('calibrationView.settingsRow.curveReady')}</p>
          ) : (
            <p className="text-xs text-text-secondary mt-1">
              {t('calibrationView.settingsRow.progress', {
                count: status.labeledTotal,
                target: status.minFitSamples,
              })}
            </p>
          ))}
      </div>
      <button
        onClick={() => setOpen(true)}
        className="px-4 py-2 text-sm border border-border rounded-md text-text-secondary hover:text-text-primary hover:bg-bg-tertiary transition-colors whitespace-nowrap shrink-0"
      >
        {t('calibrationView.settingsRow.action')}
      </button>
      <CalibrationView
        open={open}
        onClose={() => {
          setOpen(false);
          refresh();
        }}
      />
    </div>
  );
}

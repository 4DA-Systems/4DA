// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import { TasteTestStep } from '../onboarding/TasteTestStep';
import { SprintPhase } from './SprintPhase';

interface CalibrationViewProps {
  open: boolean;
  onClose: () => void;
}

type Phase = 'loading' | 'taste' | 'sprint';

/**
 * Post-onboarding calibration surface. Two phases:
 *
 * 1. The Bayesian taste test (reused in place from onboarding — its
 *    TasteTestStep is fully self-contained) for installs that never
 *    took it, including everyone who onboarded before it shipped.
 * 2. The review sprint: explicit relevant / not-relevant labels on
 *    real corpus items, which feed the calibration fitter's ground
 *    truth (50 labels unlock the first confidence-curve fit).
 *
 * Launched from Settings (CalibrationSettingsRow) and the one-time
 * nudge banner (CalibrationNudgeBanner). Deliberately NOT a nav tab —
 * the main nav is locked (intelligence doctrine rule 2).
 *
 * Rendered through a portal so the overlay escapes the Settings
 * modal's backdrop-filter containing block.
 */
export function CalibrationView({ open, onClose }: CalibrationViewProps) {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>('loading');

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setPhase('loading');
    cmd('taste_test_is_calibrated')
      .then((calibrated) => {
        if (!cancelled) setPhase(calibrated ? 'sprint' : 'taste');
      })
      .catch(() => {
        // If the check itself fails, the sprint is still usable.
        if (!cancelled) setPhase('sprint');
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  // Escape closes the overlay — except during taste cards, where the
  // embedded TasteTestStep already maps Escape to "finish early" and a
  // double-bind would skip AND close in one keystroke.
  useEffect(() => {
    if (!open || phase === 'taste') return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, phase, onClose]);

  if (!open) return null;

  return createPortal(
    <div
      className="fixed inset-0 bg-black/80 backdrop-blur-sm flex items-center justify-center z-[60] p-4"
      role="dialog"
      aria-modal="true"
      aria-label={t('calibrationView.title')}
    >
      <div className="bg-bg-primary border border-border rounded-xl w-full max-w-2xl max-h-[90vh] overflow-y-auto p-6 relative">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-base font-semibold text-text-primary">{t('calibrationView.title')}</h2>
          <button
            onClick={onClose}
            aria-label={t('action.close')}
            className="text-text-muted hover:text-text-primary transition-colors text-lg leading-none px-2 py-1"
          >
            <span aria-hidden="true">{'✕'}</span>
          </button>
        </div>

        {phase === 'loading' && (
          <div className="py-12 text-center" data-testid="calibration-loading">
            <div className="animate-spin w-7 h-7 border-2 border-white border-t-transparent rounded-full mx-auto" />
          </div>
        )}

        {phase === 'taste' && (
          <TasteTestStep
            isAnimating={false}
            onComplete={() => setPhase('sprint')}
            onSkip={() => setPhase('sprint')}
          />
        )}

        {phase === 'sprint' && <SprintPhase onClose={onClose} />}
      </div>
    </div>,
    document.body,
  );
}

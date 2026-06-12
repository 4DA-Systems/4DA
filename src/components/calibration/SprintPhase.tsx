// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { CalibrationSprintCard } from '../../../src-tauri/bindings/bindings/CalibrationSprintCard';
import type { CalibrationSprintStatus } from '../../../src-tauri/bindings/bindings/CalibrationSprintStatus';

interface SprintPhaseProps {
  onClose: () => void;
}

type SprintState = 'loading' | 'empty' | 'cards' | 'done';

/**
 * Review sprint: one real corpus item at a time, three actions.
 * Relevant / Not relevant write explicit feedback rows (the calibration
 * fitter's strongest ground truth); Skip writes nothing. The progress
 * footer shows the honest distance to the first confidence-curve fit
 * ("N of 50 labels"), read live from the backend — never fabricated.
 */
export function SprintPhase({ onClose }: SprintPhaseProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<SprintState>('loading');
  const [cards, setCards] = useState<CalibrationSprintCard[]>([]);
  const [index, setIndex] = useState(0);
  const [status, setStatus] = useState<CalibrationSprintStatus | null>(null);
  const [labeledThisSprint, setLabeledThisSprint] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([cmd('get_calibration_sprint_items'), cmd('get_calibration_sprint_status')])
      .then(([items, st]) => {
        if (cancelled) return;
        setCards(items);
        setStatus(st);
        setState(items.length > 0 ? 'cards' : 'empty');
      })
      .catch((e) => {
        if (cancelled) return;
        setError(String(e));
        setState('empty');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const advance = useCallback(() => {
    setIndex((i) => {
      if (i + 1 >= cards.length) {
        setState('done');
        // Refresh the honest total for the finish screen.
        cmd('get_calibration_sprint_status').then(setStatus).catch(() => {});
        return i;
      }
      return i + 1;
    });
  }, [cards.length]);

  const respond = useCallback(
    async (response: 'relevant' | 'not_relevant' | 'skip') => {
      const card = cards[index];
      if (!card || busy) return;
      setBusy(true);
      setError(null);
      try {
        await cmd('record_calibration_sprint_response', {
          sourceItemId: card.sourceItemId,
          response,
        });
        if (response !== 'skip') {
          setLabeledThisSprint((n) => n + 1);
          setStatus((s) => (s ? { ...s, labeledTotal: s.labeledTotal + 1 } : s));
        }
        advance();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [cards, index, busy, advance],
  );

  // Keyboard shortcuts: right/1 relevant, left/2 not relevant, space/3 skip.
  useEffect(() => {
    if (state !== 'cards') return;
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      switch (e.key) {
        case 'ArrowRight':
        case '1':
          e.preventDefault();
          void respond('relevant');
          break;
        case 'ArrowLeft':
        case '2':
          e.preventDefault();
          void respond('not_relevant');
          break;
        case ' ':
        case '3':
          e.preventDefault();
          void respond('skip');
          break;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [state, respond]);

  const target = status?.minFitSamples ?? 50;

  if (state === 'loading') {
    return (
      <div className="py-12 text-center" data-testid="sprint-loading">
        <div className="animate-spin w-7 h-7 border-2 border-white border-t-transparent rounded-full mx-auto" />
      </div>
    );
  }

  if (state === 'empty') {
    return (
      <div className="py-10 text-center space-y-3" data-testid="sprint-empty">
        <p className="text-sm text-text-secondary">{t('calibrationView.sprint.noItems')}</p>
        <p className="text-xs text-text-muted">{t('calibrationView.sprint.noItemsHint')}</p>
        {error && <p className="text-red-400 text-xs">{error}</p>}
        <button
          onClick={onClose}
          className="mt-2 px-4 py-2 text-sm border border-border rounded-md text-text-secondary hover:bg-bg-tertiary transition-colors"
        >
          {t('action.close')}
        </button>
      </div>
    );
  }

  if (state === 'done') {
    return (
      <div className="py-8 text-center space-y-4" data-testid="sprint-done">
        <h3 className="text-lg font-medium text-text-primary">{t('calibrationView.sprint.doneTitle')}</h3>
        <p className="text-sm text-text-secondary">
          {t('calibrationView.sprint.doneBody', { count: labeledThisSprint })}
        </p>
        {status && (
          <p className="text-xs text-text-muted">
            {status.curveFitted
              ? t('calibrationView.sprint.curveFitted')
              : t('calibrationView.sprint.labelProgress', {
                  count: status.labeledTotal,
                  target,
                })}
          </p>
        )}
        <p className="text-xs text-text-muted max-w-sm mx-auto">{t('calibrationView.sprint.unlocks')}</p>
        <button
          onClick={onClose}
          className="bg-accent-primary text-bg-primary font-medium text-sm py-2.5 px-6 rounded-md hover:bg-accent-primary-hover transition-colors"
        >
          {t('calibrationView.sprint.close')}
        </button>
      </div>
    );
  }

  const card = cards[index];
  if (!card) return null;

  return (
    <div className="space-y-4" data-testid="sprint-cards">
      {/* Sprint progress */}
      <div className="flex items-center gap-3">
        <div className="flex-1 bg-bg-tertiary rounded-full h-1.5 overflow-hidden">
          <div
            className="bg-accent-primary h-full rounded-full transition-all duration-300"
            style={{ width: `${Math.round(((index + 1) / cards.length) * 100)}%` }}
          />
        </div>
        <span className="text-xs text-text-muted whitespace-nowrap">
          {t('calibrationView.sprint.cardProgress', { current: index + 1, total: cards.length })}
        </span>
      </div>

      {error && <p className="text-red-400 text-xs">{error}</p>}

      {/* Card */}
      <div className="bg-bg-secondary border border-border rounded-lg p-6">
        <div className="flex items-center justify-between mb-4">
          <span className="text-[11px] text-text-muted bg-bg-tertiary px-2 py-0.5 rounded">
            {card.sourceType}
          </span>
        </div>
        <h3 className="text-text-primary font-medium text-base mb-3 leading-snug">{card.title}</h3>
        {card.snippet && (
          <p className="text-text-secondary text-sm leading-relaxed mb-6 line-clamp-3">
            {card.snippet}
          </p>
        )}

        <div className="flex items-center gap-3">
          <button
            onClick={() => {
              void respond('relevant');
            }}
            disabled={busy}
            className="flex-1 bg-accent-primary text-bg-primary font-medium text-sm py-2.5 px-4 rounded-md hover:bg-accent-primary-hover transition-colors disabled:opacity-50"
          >
            {t('calibrationView.sprint.relevant')}
          </button>
          <button
            onClick={() => {
              void respond('not_relevant');
            }}
            disabled={busy}
            className="flex-1 border border-border text-text-secondary text-sm py-2.5 px-4 rounded-md hover:bg-bg-tertiary transition-colors disabled:opacity-50"
          >
            {t('calibrationView.sprint.notRelevant')}
          </button>
          <button
            onClick={() => {
              void respond('skip');
            }}
            disabled={busy}
            className="px-4 py-2.5 text-sm text-text-muted hover:text-text-secondary transition-colors disabled:opacity-50"
          >
            {t('calibrationView.sprint.skip')}
          </button>
        </div>
      </div>

      {/* Honest fit progress + keyboard hint */}
      <div className="text-center space-y-1">
        {status && !status.curveFitted && (
          <p className="text-xs text-text-muted">
            {t('calibrationView.sprint.labelProgress', { count: status.labeledTotal, target })}
          </p>
        )}
        <p className="text-[10px] text-text-muted/60">{t('calibrationView.sprint.keyboardHint')}</p>
      </div>
    </div>
  );
}

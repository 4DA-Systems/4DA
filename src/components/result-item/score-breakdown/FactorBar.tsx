import { memo, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  EFFECT_COLORS,
  formatFactorValue,
  getBarWidth,
  type Factor,
} from './factor-utils';

interface FactorBarProps {
  factor: Factor;
  compareValue?: number;
  itemId: number;
  onFeedbackGiven: (factorKey: string, vote: 'up' | 'down') => void;
}

export const FactorBar = memo(function FactorBar({
  factor,
  compareValue,
  itemId,
  onFeedbackGiven,
}: FactorBarProps) {
  const { t } = useTranslation();
  const [feedbackGiven, setFeedbackGiven] = useState<'up' | 'down' | null>(null);
  const colors = EFFECT_COLORS[factor.effect];
  const barWidth = getBarWidth(factor);
  const factorLabel = t(factor.labelKey, factor.label);

  const handleFeedback = useCallback(async (vote: 'up' | 'down') => {
    setFeedbackGiven(vote);
    try {
      await invoke('ace_record_interaction', {
        itemId,
        actionType: vote === 'up' ? 'click' : 'dismiss',
        actionData: { factor: factor.key, dwell_time_seconds: 0 },
        itemTopics: [factor.key],
        itemSource: 'score_feedback',
      });
      onFeedbackGiven(factor.key, vote);
    } catch {
      // Feedback is best-effort
    }
  }, [itemId, factor.key, onFeedbackGiven]);

  return (
    <div className="group flex items-center gap-2">
      {/* Label */}
      <span className={`text-[11px] w-28 flex-shrink-0 ${colors.label}`}>
        {factorLabel}
      </span>

      {/* Bar container */}
      <div className="flex-1 h-4 bg-bg-tertiary rounded overflow-hidden relative">
        <div
          className={`h-full rounded transition-all duration-300 ${colors.bar}`}
          style={{ width: `${barWidth}%` }}
        />
        {/* Compare overlay */}
        {compareValue != null && (
          <div
            className="absolute top-0 h-full border-r-2 border-white/40"
            style={{ left: `${getBarWidth({ ...factor, value: compareValue })}%` }}
          />
        )}
      </div>

      {/* Value */}
      <span className={`text-[11px] font-mono w-12 text-right flex-shrink-0 ${colors.text}`}>
        {formatFactorValue(factor)}
      </span>

      {/* Teach Me — thumbs up/down (only visible on hover) */}
      <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
        {feedbackGiven ? (
          <span className="text-[10px] text-text-muted w-8 text-center">
            {feedbackGiven === 'up' ? '\u2713' : '\u2717'}
          </span>
        ) : (
          <>
            <button
              onClick={() => handleFeedback('up')}
              className="text-[10px] text-text-muted hover:text-green-400 transition-colors px-0.5"
              title="This factor was relevant"
              aria-label={`${factorLabel} was relevant`}
            >
              +
            </button>
            <button
              onClick={() => handleFeedback('down')}
              className="text-[10px] text-text-muted hover:text-amber-400 transition-colors px-0.5"
              title="This factor wasn't relevant"
              aria-label={`${factorLabel} was not relevant`}
            >
              &minus;
            </button>
          </>
        )}
      </div>

      {/* Detail text */}
      {factor.detail && (
        <span className="text-[9px] text-text-muted truncate max-w-[100px] flex-shrink-0" title={factor.detail}>
          {factor.detail}
        </span>
      )}
    </div>
  );
});

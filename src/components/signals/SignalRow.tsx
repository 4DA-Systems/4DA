// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getSourceLabel } from '../../config/sources';
import { getRelevancePresentation } from '../../utils/score';
import { SIGNAL_CONFIG, PRIORITY_CONFIG, SIGNAL_LABELS } from './signal-config';

export interface SignalItem {
  id: number;
  title: string;
  url: string | null;
  top_score: number;
  source_type: string;
  signal_type: string;
  signal_priority: string;
  signal_action: string;
  signal_triggers: string[];
  similar_count: number;
  similar_titles: string[];
}

export const SignalRow = ({ signal }: { signal: SignalItem }) => {
  const { t } = useTranslation();
  const [showTriggers, setShowTriggers] = useState(false);
  const config = (SIGNAL_CONFIG[signal.signal_type] ?? SIGNAL_CONFIG['tech_trend'])!;
  const priority = (PRIORITY_CONFIG[signal.signal_priority] ?? PRIORITY_CONFIG['watch'])!;
  const sourceLabel = getSourceLabel(signal.source_type);

  return (
    <div
      className={`px-4 py-3 rounded-lg border ${config.borderColor} ${config.bgColor} transition-all hover:brightness-125`}
    >
      <div className="flex items-start gap-3">
        {/* Icon + Priority */}
        <div className="flex flex-col items-center gap-1 pt-0.5" aria-hidden="true">
          <span className="text-base">{config.icon}</span>
          <span className={`w-2 h-2 rounded-full ${priority.dot}`} />
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          {/* Top row: action text */}
          <p className={`text-sm font-medium ${config.color} leading-snug`}>
            {signal.signal_action}
          </p>

          {/* Title + meta */}
          <div className="flex items-center gap-2 mt-1">
            {signal.url ? (
              <a
                href={signal.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-text-secondary hover:text-text-primary truncate transition-colors"
                title={signal.title}
              >
                {signal.title}
              </a>
            ) : (
              <span className="text-xs text-text-secondary truncate">{signal.title}</span>
            )}
          </div>

          {/* Bottom row: type badge + priority + score + triggers */}
          <div className="flex items-center gap-2 mt-2">
            <span className={`px-1.5 py-0.5 text-[10px] rounded ${config.bgColor} ${config.color} border ${config.borderColor}`}>
              {SIGNAL_LABELS[signal.signal_type] ?? signal.signal_type}
            </span>
            <span className={`px-1.5 py-0.5 text-[10px] font-medium rounded ${priority.bgColor} ${priority.color}`}>
              {priority.label}
            </span>
            <span className={`text-[10px] uppercase tracking-wider ${getRelevancePresentation(signal.top_score).colorClass}`}>
              {t(getRelevancePresentation(signal.top_score).labelKey)}
            </span>
            <span className="text-[10px] text-text-muted">{sourceLabel}</span>
            {signal.signal_triggers.length > 0 && (
              <button
                onClick={() => setShowTriggers(!showTriggers)}
                aria-expanded={showTriggers}
                aria-label={showTriggers ? t('signals.hideTriggers') : t('signals.triggers', { count: signal.signal_triggers.length })}
                className="text-[10px] text-text-muted hover:text-text-secondary transition-colors ms-auto"
              >
                {showTriggers ? t('signals.hideTriggers') : t('signals.triggers', { count: signal.signal_triggers.length })}
              </button>
            )}
          </div>

          {/* Similar items grouped */}
          {signal.similar_count > 0 && (
            <div className="mt-1.5 text-[10px] text-text-muted">
              {t('signals.similar', { count: signal.similar_count })}{signal.similar_titles.length > 0 && (
                <span className="text-text-muted"> ({signal.similar_titles.slice(0, 2).join(', ')}{signal.similar_titles.length > 2 ? '...' : ''})</span>
              )}
            </div>
          )}

          {/* Trigger keywords */}
          {showTriggers && (
            <div className="flex flex-wrap gap-1 mt-2">
              {signal.signal_triggers.map((t, i) => (
                <span key={i} className="px-1.5 py-0.5 text-[10px] bg-bg-tertiary text-text-secondary rounded border border-border">
                  {t}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

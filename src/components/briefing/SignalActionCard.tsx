import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';

interface SignalActionCardProps {
  item: SourceRelevance;
  feedbackGiven?: FeedbackAction;
  onSave: (item: SourceRelevance) => void;
  onDismiss: (item: SourceRelevance) => void;
}

const priorityStyles: Record<string, { border: string; bg: string; label: string; text: string }> = {
  critical: {
    border: 'border-red-500/30',
    bg: 'bg-red-500/5',
    label: 'CRITICAL',
    text: 'text-red-400',
  },
  high: {
    border: 'border-orange-500/30',
    bg: 'bg-orange-500/5',
    label: 'HIGH',
    text: 'text-orange-400',
  },
};

const signalTypeLabels: Record<string, string> = {
  security: 'SECURITY',
  breaking_change: 'BREAKING',
  deprecation: 'DEPRECATION',
  vulnerability: 'SECURITY',
  release: 'RELEASE',
  trending: 'TRENDING',
};

export const SignalActionCard = memo(function SignalActionCard({
  item,
  feedbackGiven,
  onSave,
  onDismiss,
}: SignalActionCardProps) {
  const { t } = useTranslation();
  const priority = item.signal_priority || 'high';
  const style = priorityStyles[priority] || priorityStyles.high;
  const signalLabel = signalTypeLabels[item.signal_type || ''] || item.signal_type?.toUpperCase() || 'SIGNAL';
  const source = item.source_type || 'hackernews';

  return (
    <div className={`rounded-lg border ${style.border} ${style.bg} p-4`}>
      <div className="flex items-start gap-3">
        <div className="flex flex-col items-start gap-1.5 flex-shrink-0">
          <span className={`text-[10px] px-2 py-0.5 rounded font-bold tracking-wider ${style.text} ${style.bg} border ${style.border}`}>
            {signalLabel}
          </span>
          <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(source)}`}>
            {getSourceLabel(source)}
          </span>
        </div>

        <div className="flex-1 min-w-0">
          {item.signal_action ? (
            <p className={`text-sm font-medium ${style.text}`}>{item.signal_action}</p>
          ) : (
            <p className="text-sm font-medium text-white">{item.title}</p>
          )}
          {item.signal_action && item.title !== item.signal_action && (
            <p className="text-xs text-gray-400 mt-1">{item.title}</p>
          )}
          {item.explanation && (
            <p className="text-xs text-gray-500 mt-1.5 leading-relaxed">{item.explanation}</p>
          )}
        </div>

        <div className="flex items-center gap-1.5 flex-shrink-0">
          {feedbackGiven ? (
            <span className={`text-xs px-2 py-1 rounded ${
              feedbackGiven === 'save' ? 'bg-green-500/20 text-green-400' : 'bg-gray-500/20 text-gray-500'
            }`}>
              {feedbackGiven === 'save' ? t('feedback.saved') : t('feedback.dismissed')}
            </span>
          ) : (
            <>
              {item.url && (
                <button
                  onClick={() => window.open(item.url!, '_blank', 'noopener,noreferrer')}
                  className="px-2.5 py-1.5 text-xs bg-bg-tertiary text-gray-300 border border-border rounded-lg hover:bg-border transition-all font-medium"
                >
                  {t('briefing.read')}
                </button>
              )}
              <button
                onClick={() => onSave(item)}
                className="px-2.5 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/20 rounded-lg hover:bg-green-500/20 transition-all font-medium"
              >
                {t('action.save')}
              </button>
              <button
                onClick={() => onDismiss(item)}
                className="px-2.5 py-1.5 text-xs bg-border text-gray-500 border border-[#333] rounded-lg hover:bg-red-500/10 hover:text-red-400 hover:border-red-500/20 transition-all font-medium"
              >
                {t('action.dismiss')}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
});

import { useTranslation } from 'react-i18next';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { Recommendation } from '../../types/calibration';
import { priorityColor } from './calibration-utils';

export function DimensionBar({ label, score }: { label: string; score: number }) {
  const pct = Math.round((score / 25) * 100);
  const color = pct >= 80 ? '#22C55E' : pct >= 50 ? '#D4AF37' : pct >= 25 ? '#F59E0B' : '#EF4444';
  return (
    <div>
      <div className="flex justify-between mb-0.5">
        <span className="text-[11px] text-text-secondary">{label}</span>
        <span className="text-[11px] font-mono" style={{ color }}>{score}/25</span>
      </div>
      <div
        className="h-1 bg-border rounded-sm overflow-hidden"
        role="progressbar"
        aria-valuenow={score}
        aria-valuemin={0}
        aria-valuemax={25}
        aria-label={`${label}: ${score} out of 25`}
      >
        <div
          className="h-full rounded-sm transition-[width] duration-300 ease-in-out"
          style={{ width: `${pct}%`, background: color }}
        />
      </div>
    </div>
  );
}

export function StatusRow({ label, ok, detail }: { label: string; ok: boolean; detail: string }) {
  return (
    <div className="flex items-center gap-2" role="status" aria-label={`${label}: ${detail}`}>
      <span className={`text-sm ${ok ? 'text-success' : 'text-error'}`}>
        {ok ? '\u2713' : '\u2717'}
      </span>
      <span className="text-xs text-text-secondary">{label}:</span>
      <span className="text-xs text-white font-mono">
        {detail}
      </span>
    </div>
  );
}

interface ActionButtonProps {
  rec: Recommendation;
  actionInProgress: string | null;
  onAction: (rec: Recommendation) => void;
}

export function ActionButton({ rec, actionInProgress, onAction }: ActionButtonProps) {
  const { t } = useTranslation();

  if (!rec.action_type) return null;

  const isActive = actionInProgress === rec.action_type;
  const labels: Record<string, { idle: string; active: string }> = {
    pull_embedding_model: { idle: t('calibration.action.pullModel'), active: t('calibration.action.pulling') },
    open_settings_interests: { idle: t('calibration.action.openSettings'), active: t('calibration.action.openSettings') },
    open_settings_stacks: { idle: t('calibration.action.openSettings'), active: t('calibration.action.openSettings') },
    auto_detect_stacks: { idle: t('calibration.action.autoDetect'), active: t('calibration.action.detecting') },
    install_ollama: { idle: t('calibration.action.installOllama'), active: t('calibration.action.installing') },
    give_feedback: { idle: t('calibration.action.giveFeedback'), active: t('calibration.action.giveFeedback') },
  };
  const label = labels[rec.action_type] || { idle: t('calibration.action.fix'), active: t('calibration.action.fixing') };

  return (
    <button
      onClick={() => onAction(rec)}
      disabled={!!actionInProgress && !['open_settings_interests', 'open_settings_stacks', 'give_feedback', 'install_ollama'].includes(rec.action_type!)}
      aria-label={isActive ? label.active : label.idle}
      className={`mt-1.5 px-3 py-1 border-none rounded text-[11px] font-semibold font-[Inter,sans-serif] ${
        isActive
          ? 'bg-border text-text-muted cursor-not-allowed'
          : 'bg-accent-gold text-bg-primary cursor-pointer'
      }`}
    >
      {isActive ? label.active : label.idle}
    </button>
  );
}

interface RecommendationItemProps {
  rec: Recommendation;
  index: number;
  actionInProgress: string | null;
  onAction: (rec: Recommendation) => void;
}

export function RecommendationItem({ rec, index, actionInProgress, onAction }: RecommendationItemProps) {
  const { getTranslated } = useTranslatedContent();
  const recId = rec.action_type ?? `rec-${index}`;
  return (
    <div className={`py-2 ${index > 0 ? 'border-t border-bg-tertiary' : ''}`}>
      <div className="flex items-center gap-2">
        <span
          className="text-[10px] font-bold font-mono"
          style={{ color: priorityColor(rec.priority) }}
        >
          {rec.priority}
        </span>
        <span className="text-[13px] font-medium text-white">
          {getTranslated(`cal-title-${recId}`, rec.title)}
        </span>
      </div>
      <p className="mt-1 mb-0 text-xs text-text-secondary leading-[1.4]">
        {getTranslated(`cal-desc-${recId}`, rec.description)}
      </p>
      <div className="flex items-center gap-2 mt-1">
        <ActionButton rec={rec} actionInProgress={actionInProgress} onAction={onAction} />
        {rec.action && !rec.action_type && (
          <code className="inline-block px-1.5 py-0.5 bg-bg-tertiary rounded-[3px] text-[11px] text-accent-gold font-mono">
            {rec.action}
          </code>
        )}
      </div>
    </div>
  );
}

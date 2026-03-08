import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { trackEvent } from '../../hooks/use-telemetry';

interface SynthesisPanelProps {
  query: string;
  isPro: boolean;
  synthesis: string | null;
  loading: boolean;
  onRetry: () => void;
}

export function SynthesisPanel({ isPro, synthesis, loading, onRetry }: SynthesisPanelProps) {
  const { t } = useTranslation();

  // Track when synthesis loading begins
  useEffect(() => {
    if (loading) trackEvent('synthesis_triggered');
  }, [loading]);

  if (!isPro) return null;
  if (!loading && !synthesis) return null;

  return (
    <div className="rounded-lg bg-cyan-500/5 border border-cyan-500/20 p-4" role="region" aria-label={t('search.aiSynthesis')} aria-live="polite">
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] text-cyan-400 uppercase tracking-wider font-medium">
          {t('search.aiSynthesis')}
        </span>
        {synthesis && !loading && (
          <button
            onClick={onRetry}
            className="text-[10px] text-text-muted hover:text-cyan-400 transition-colors"
          >
            {t('action.retry')}
          </button>
        )}
      </div>

      {loading ? (
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-cyan-400 animate-pulse" />
          <span className="text-sm text-text-secondary">{t('search.analyzingSignals')}</span>
        </div>
      ) : (
        <p className="text-sm text-text-secondary leading-relaxed whitespace-pre-wrap">{synthesis}</p>
      )}
    </div>
  );
}

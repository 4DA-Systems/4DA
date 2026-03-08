import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { trackEvent } from '../../hooks/use-telemetry';

export interface SynthesisSource {
  index: number;
  title: string;
  url: string | null;
  source_type: string;
}

export interface SynthesisResponse {
  text: string;
  sources: SynthesisSource[];
  grounding_count: number;
  total_sources: number;
}

interface SynthesisPanelProps {
  query: string;
  isPro: boolean;
  synthesis: SynthesisResponse | null;
  loading: boolean;
  streamingText?: string;
  onRetry: () => void;
}

function parseCitations(
  text: string,
  sources: SynthesisSource[],
): React.ReactNode[] {
  const parts = text.split(/(\[\d+\])/g);
  return parts.map((part, i) => {
    const match = part.match(/^\[(\d+)\]$/);
    if (match) {
      const idx = parseInt(match[1], 10);
      const source = sources.find(s => s.index === idx);
      if (source?.url) {
        return (
          <a
            key={i}
            href={source.url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center justify-center w-4 h-4 text-[9px] bg-cyan-500/20 text-cyan-400 rounded-sm hover:bg-cyan-500/30 transition-colors align-super ml-0.5 mr-0.5 no-underline"
            title={source.title}
          >
            {idx}
          </a>
        );
      }
      return (
        <span key={i} className="text-[9px] text-cyan-400/60 align-super ml-0.5 mr-0.5" title={source?.title}>
          [{idx}]
        </span>
      );
    }
    return <span key={i}>{part}</span>;
  });
}

export function SynthesisPanel({ isPro, synthesis, loading, streamingText, onRetry }: SynthesisPanelProps) {
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

      {loading && streamingText ? (
        <div>
          <p className="text-sm text-text-secondary leading-relaxed">
            {streamingText}
            <span className="inline-block w-0.5 h-4 bg-cyan-400 ml-0.5 animate-pulse" />
          </p>
          <div className="flex items-center gap-2 mt-2">
            <span className="w-2 h-2 rounded-full bg-cyan-400 animate-pulse" />
            <span className="text-[10px] text-text-muted">{t('search.synthesizing')}</span>
          </div>
        </div>
      ) : loading ? (
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-cyan-400 animate-pulse" />
          <span className="text-sm text-text-secondary">{t('search.analyzingSignals')}</span>
        </div>
      ) : (
        <>
          <p className="text-sm text-text-secondary leading-relaxed">
            {parseCitations(synthesis!.text, synthesis!.sources)}
          </p>

          {/* Grounding indicator */}
          {synthesis!.total_sources > 0 && (
            <div className="flex items-center gap-2 mt-2 pt-2 border-t border-cyan-500/10">
              <div className="flex gap-0.5">
                {Array.from({ length: synthesis!.total_sources }, (_, i) => (
                  <div
                    key={i}
                    className={`w-1.5 h-1.5 rounded-full ${
                      i < synthesis!.grounding_count ? 'bg-cyan-400' : 'bg-cyan-400/20'
                    }`}
                  />
                ))}
              </div>
              <span className={`text-[10px] ${synthesis!.grounding_count === 0 ? 'text-amber-400' : 'text-text-muted'}`}>
                {synthesis!.grounding_count === 0
                  ? t('search.ungrounded')
                  : t('search.groundedIn', { count: synthesis!.grounding_count, total: synthesis!.total_sources })}
              </span>
            </div>
          )}

          {/* Sources list */}
          {synthesis!.sources.length > 0 && (
            <details className="mt-2">
              <summary className="text-[10px] text-text-muted cursor-pointer hover:text-text-secondary select-none">
                {t('search.viewSources', { count: synthesis!.sources.length })}
              </summary>
              <div className="mt-1 space-y-0.5">
                {synthesis!.sources.map(source => (
                  <div key={source.index} className="flex items-center gap-1.5 text-[10px]">
                    <span className="text-cyan-400/60 w-3 text-right">{source.index}.</span>
                    {source.url ? (
                      <a
                        href={source.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-text-secondary hover:text-cyan-400 transition-colors truncate"
                      >
                        {source.title}
                      </a>
                    ) : (
                      <span className="text-text-muted truncate">{source.title}</span>
                    )}
                  </div>
                ))}
              </div>
            </details>
          )}
        </>
      )}
    </div>
  );
}

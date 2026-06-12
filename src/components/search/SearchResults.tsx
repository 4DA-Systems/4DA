// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';
import { formatLocalDate } from '../../utils/format-date';
import { SynthesisPanel, type SynthesisResponse } from './SynthesisPanel';
import { GhostPreview } from './GhostPreview';
import type { QueryResult } from './search-types';
import { intentLabels, sourceLabels } from './search-types';

export interface SearchResultsProps {
  query: string;
  result: QueryResult;
  isPro: boolean;
  synthesis: SynthesisResponse | null;
  synthesisLoading: boolean;
  streamingText: string;
  onRetrySynthesis: () => void;
  onClear: () => void;
}

export function SearchResults({
  query,
  result,
  isPro,
  synthesis,
  synthesisLoading,
  streamingText,
  onRetrySynthesis,
  onClear,
}: SearchResultsProps) {
  const { t } = useTranslation();
  const relevantStack = result.stack_context?.filter((s) => s.relevant) ?? [];

  return (
    <div className="space-y-4">
      {/* Synthesis panel (every tier — BYOK, runs on the user's own key) */}
      <SynthesisPanel query={query} synthesis={synthesis} loading={synthesisLoading} streamingText={streamingText} onRetry={onRetrySynthesis} />

      {/* Query parsing info */}
      <div className="flex items-center gap-2 p-3 bg-bg-secondary rounded-lg border border-border flex-wrap">
        <span className="text-xs font-medium text-text-muted">{intentLabels[result.intent] || result.intent}</span>
        <span className="text-sm text-text-primary">{'•'}</span>
        <span className="text-sm text-cyan-400">{result.parsed.keywords.join(', ')}</span>
        {result.parsed.time_range && (
          <span className="px-2 py-1 text-xs bg-bg-tertiary rounded-md text-text-secondary border border-border">
            {result.parsed.time_range.relative || t('search.customRange')}
          </span>
        )}
        {result.parsed.file_types.length > 0 && (
          <span className="px-2 py-1 text-xs bg-bg-tertiary rounded-md text-text-secondary border border-border">
            {result.parsed.file_types.join(', ')}
          </span>
        )}
        <button onClick={onClear} aria-label="Clear search results" className="ms-auto text-text-muted hover:text-text-primary transition-colors">{'✕'}</button>
      </div>

      {/* Stack context */}
      {relevantStack.length > 0 && (
        <div className="flex items-center gap-2 text-xs text-text-secondary">
          <span className="text-text-muted">{t('search.yourStack')}:</span>
          {relevantStack.map((s) => (
            <span key={s.name} className="px-2 py-0.5 bg-bg-secondary rounded border border-border text-text-secondary">{s.name}</span>
          ))}
        </div>
      )}

      {/* Summary */}
      {result.summary && (
        <div className="text-sm text-text-secondary bg-bg-secondary rounded-lg p-4 border border-border">{result.summary}</div>
      )}

      {/* Result items */}
      <div className="space-y-2 max-h-64 overflow-y-auto" role="list" aria-label="Search results">
        {result.items.map((item, index) => (
          <div key={`${item.id}-${index}`} role="listitem" className="p-3 bg-bg-secondary rounded-lg border border-border hover:border-cyan-500/30 transition-colors">
            <div className="flex items-start gap-3">
              <span className="text-[10px] text-text-muted uppercase font-mono bg-bg-tertiary px-1.5 py-0.5 rounded">{sourceLabels[item.source_type] || 'SRC'}</span>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm text-text-primary font-medium truncate">{item.file_name || t('search.unknownFile')}</span>
                  <span
                    className={`text-xs px-2 py-0.5 rounded-md ${item.relevance > 0.7 ? 'bg-green-500/20 text-green-400' : item.relevance > 0.4 ? 'bg-yellow-500/20 text-yellow-400' : 'bg-gray-500/20 text-text-secondary'}`}
                    aria-label={`Relevance: ${(item.relevance * 100).toFixed(0)}%`}
                  >
                    {(item.relevance * 100).toFixed(0)}%
                  </span>
                </div>
                <p className="text-xs text-text-muted mt-1 line-clamp-2">{item.preview}</p>
                <div className="flex items-center gap-2 mt-2 text-[10px] text-text-muted">
                  <span className="text-cyan-400/70">{item.match_reason}</span>
                  {item.timestamp && (
                    <>
                      <span>{'•'}</span>
                      <span>{formatLocalDate(new Date(item.timestamp))}</span>
                    </>
                  )}
                </div>
              </div>
            </div>
          </div>
        ))}
        {result.items.length === 0 && (
          <div className="text-center py-6 bg-bg-secondary rounded-lg border border-border">
            <div className="text-sm text-text-secondary">{t('search.noResults')}</div>
            <div className="text-xs text-text-muted mt-1">{t('search.tryDifferent')}</div>
          </div>
        )}
      </div>

      {/* Related decisions (Pro) */}
      {isPro && result.related_decisions.length > 0 && (
        <div className="space-y-1.5">
          <h4 className="text-xs text-text-secondary uppercase tracking-wider">{t('search.relatedDecisions')}</h4>
          {result.related_decisions.map((d) => (
            <div key={d.id} className="px-3 py-2 bg-bg-secondary rounded-lg border border-border text-xs">
              <span className="text-text-secondary">{d.subject}</span>
              <span className="text-text-muted mx-1.5">{'—'}</span>
              <span className="text-text-secondary">{d.decision}</span>
            </div>
          ))}
        </div>
      )}

      {/* Knowledge gaps (Pro) */}
      {isPro && result.knowledge_gaps.length > 0 && (
        <div className="space-y-1.5">
          <h4 className="text-xs text-text-secondary uppercase tracking-wider">{t('search.knowledgeGaps')}</h4>
          {result.knowledge_gaps.map((gap, i) => (
            <div key={i} className="flex items-center gap-2 px-3 py-2 bg-bg-secondary rounded-lg border border-border text-xs">
              <span className={gap.severity === 'critical' ? 'text-red-400' : gap.severity === 'high' ? 'text-yellow-400' : 'text-text-secondary'}>{'●'}</span>
              <span className="text-text-secondary">{gap.technology}</span>
              <span className="text-text-muted">{t('search.staleForDays', { days: gap.days_stale })}</span>
            </div>
          ))}
        </div>
      )}

      {/* Ghost preview (non-Pro insight) */}
      {result.ghost_preview && !result.is_pro && (
        <GhostPreview preview={result.ghost_preview} />
      )}

      {/* Stats footer */}
      <div className="text-xs text-text-muted text-center pt-2 border-t border-border">
        {t('search.stats', { count: result.total_count, ms: result.execution_ms, confidence: (result.parsed.confidence * 100).toFixed(0) })}
      </div>
    </div>
  );
}

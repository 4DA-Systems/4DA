// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance, FeedbackAction } from '../../types';
import { formatRelativeAge, getScoreFactorKeys, getScoreChipKeys, getRelevancePresentation } from '../../utils/score';
import { getSourceLabel, getSourceColorClass } from '../../config/sources';
import { isSafeUrl } from '../../utils/sanitize-html';
import { formatLocalDateTime } from '../../utils/format-date';
import { useTranslatedContent } from '../ContentTranslationProvider';
import { cmd } from '../../lib/commands';
import { extractTechTopics } from '../../lib/known-tech';

interface ResultItemCollapsedProps {
  item: SourceRelevance;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onToggleBreakdown?: () => void;
  showBreakdown?: boolean;
  feedback: FeedbackAction | undefined;
  fallbackReason: string;
}

/**
 * Compact collapsed result item.
 * Default: score + source + title on one line.
 * Badges & explanations only appear on expand.
 */
export const ResultItemCollapsed = memo(function ResultItemCollapsed({
  item,
  isExpanded,
  onToggleExpand,
  onToggleBreakdown,
  showBreakdown,
  feedback,
  fallbackReason,
}: ResultItemCollapsedProps) {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();
  const displayTitle = getTranslated(String(item.id), item.title);
  const expandedReason = !item.explanation ? fallbackReason : '';
  const relevance = getRelevancePresentation(item.top_score);
  const scoreTooltip = useMemo(() => {
    const factors = item.score_breakdown?.explanation_factors;
    if (factors?.length) {
      // Tooltip shows the full chain: named display + its concrete evidence.
      return factors.map(f => `${f.display} — ${f.evidence}`).join('\n');
    }
    const keys = getScoreFactorKeys(item);
    if (keys.length === 0) return undefined;
    return keys.map(k => t(k)).join('\n');
  }, [item, t]);
  // Rendering contract: this dense list row surfaces evidence AS chips, so the
  // STRONGEST factor must lead as the first chip (chain is trust-ordered, so
  // factor[0] is the highest-trust evidence — "Names your dependency axios",
  // "Security advisory affects your dependency X"). The full prose + evidence
  // is in the score tooltip and the expanded EvidenceChain. "+N more" appears
  // ONLY as a suffix to named factor chips — never a bare count. Legacy generic
  // chips remain solely for chain-less items.
  const factorChips = useMemo(() => {
    const factors = item.score_breakdown?.explanation_factors;
    if (!factors?.length) return null;
    const chips = factors.slice(0, 3).map(f => f.display);
    const remaining = factors.length - 3;
    return { chips, remaining: remaining > 0 ? remaining : 0 };
  }, [item.score_breakdown?.explanation_factors]);
  const chipKeys = useMemo(
    () => (factorChips ? [] : getScoreChipKeys(item)),
    [factorChips, item],
  );

  const recordTitleClick = useCallback(() => {
    const topics = extractTechTopics(item.title);
    cmd('ace_record_interaction', {
      itemId: item.id,
      actionType: 'click',
      actionData: JSON.stringify({
        type: 'click',
        dwell_time_seconds: 0,
        pattern: 'engaged',
      }),
      itemTopics: topics,
      itemSource: item.source_type || 'unknown',
    }).catch(() => {});
  }, [item.id, item.title, item.source_type]);

  return (
    <div className="w-full px-4 py-2.5">
      {/* Primary row: score + source + title + age + expand */}
      <div className="flex items-center gap-3">
        {/* Score badge — click to toggle breakdown */}
        <button
          onClick={onToggleBreakdown && item.score_breakdown ? onToggleBreakdown : onToggleExpand}
          aria-expanded={showBreakdown}
          aria-label={item.score_breakdown ? `${t('scoreDrawer.toggle', 'Toggle score breakdown')}, ${t(relevance.ariaLabelKey)}` : t(relevance.ariaLabelKey)}
          title={scoreTooltip}
          className={`flex-shrink-0 w-14 text-center py-0.5 rounded text-[10px] font-medium uppercase tracking-wider cursor-pointer transition-all ${relevance.colorClass} ${showBreakdown ? 'ring-1 ring-white/30' : ''} ${item.score_breakdown ? 'hover:ring-1 hover:ring-white/20' : ''}`}
        >
          {t(relevance.labelKey)}
        </button>

        {/* Source badge */}
        <span className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded font-medium ${getSourceColorClass(item.source_type || '')}`}>
          {getSourceLabel(item.source_type || '') || item.source_type || t('results.unknownSource')}
        </span>

        {/* Signal dot */}
        {item.signal_type && (
          <span className={`flex-shrink-0 w-1.5 h-1.5 rounded-full ${
            item.signal_priority === 'critical' ? 'bg-red-400' :
            item.signal_priority === 'alert' ? 'bg-orange-400' :
            item.signal_priority === 'advisory' ? 'bg-amber-400' :
            'bg-blue-400'
          }`} title={item.signal_type} role="img" aria-label={`${item.signal_priority || 'normal'} priority: ${item.signal_type}`} />
        )}

        {/* Title */}
        <div className="flex-1 min-w-0">
          {item.url && isSafeUrl(item.url) ? (
            <a
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              onClick={(e) => { e.stopPropagation(); recordTitleClick(); }}
              aria-label={`${displayTitle} (opens in new tab)`}
              className={`text-sm truncate block hover:underline decoration-gray-600 ${
                item.relevant ? 'text-text-primary' : 'text-text-secondary'
              }`}
            >
              {displayTitle}
            </a>
          ) : (
            <button
              onClick={onToggleExpand}
              aria-label={`Expand details: ${item.title}`}
              className={`text-sm truncate block text-start w-full ${
                item.relevant ? 'text-text-primary' : 'text-text-secondary'
              }`}
            >
              {displayTitle}
            </button>
          )}
        </div>

        {/* Reason chips AFTER the title (2026-07-14 layout re-prioritization):
            the payload (what is this item?) leads the scan line; evidence
            (why is it here?) is secondary metadata. Chip contract unchanged:
            strongest factor first, "+N more" only as a suffix to named chips,
            legacy generic chips only for chain-less items. */}
        {factorChips && factorChips.chips.map(display => (
          <span
            key={display}
            className="flex-shrink-0 text-[9px] px-1.5 py-0.5 rounded bg-text-primary/[0.04] text-text-muted border border-border/40 max-w-[150px] truncate hidden md:inline-block"
            title={display}
          >
            {display}
          </span>
        ))}
        {factorChips && factorChips.remaining > 0 && (
          <span className="flex-shrink-0 text-[9px] px-1.5 py-0.5 rounded text-text-muted/70 hidden md:inline-block">
            {t('result.moreFactors', { count: factorChips.remaining })}
          </span>
        )}
        {/* Legacy generic chips — chain-less items only */}
        {!factorChips && chipKeys.length > 0 && chipKeys.map(k => (
          <span
            key={k}
            className="flex-shrink-0 text-[9px] px-1.5 py-0.5 rounded bg-text-primary/[0.04] text-text-muted border border-border/40 hidden md:inline-block"
          >
            {t(k)}
          </span>
        ))}

        {/* Advisory stack: siblings for the same package collapsed behind
            this representative — expand to see them. */}
        {(item.advisory_stack_count ?? 0) > 0 && (
          <span
            className="flex-shrink-0 text-[9px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20"
            title={t('results.advisoryStackTitle')}
          >
            {t('results.advisoryStack', { count: item.advisory_stack_count })}
          </span>
        )}

        {/* Signal strength: micro-dots showing independent confirmation axes */}
        {item.score_breakdown && (item.score_breakdown.signal_count ?? 0) > 0 && (
          <span
            className="flex-shrink-0 flex gap-px"
            title={t('results.signalStrength', { count: item.score_breakdown.signal_count })}
            aria-label={t('results.signalStrength', { count: item.score_breakdown.signal_count })}
          >
            {[0, 1, 2, 3, 4].map(i => (
              <span
                key={i}
                className={`w-1 h-1 rounded-full ${
                  i < (item.score_breakdown?.signal_count ?? 0)
                    ? (item.score_breakdown?.signal_count ?? 0) >= 4 ? 'bg-green-400' : 'bg-text-muted'
                    : 'bg-text-primary/[0.06]'
                }`}
              />
            ))}
          </span>
        )}

        {/* Age */}
        {item.created_at && (
          <span className="flex-shrink-0 text-[10px] text-text-muted/60" title={formatLocalDateTime(item.created_at)}>
            {formatRelativeAge(item.created_at)}
          </span>
        )}

        {/* Feedback indicator */}
        {feedback && (
          <span
            className={`flex-shrink-0 text-[10px] px-1.5 py-0.5 rounded ${
              feedback === 'save'
                ? 'bg-success/20 text-success'
                : feedback === 'dismiss'
                ? 'bg-text-muted/20 text-text-muted'
                : 'bg-error/20 text-error'
            }`}
          >
            {feedback === 'save'
              ? `\u2713 ${t('feedback.saved')}`
              : feedback === 'dismiss'
              ? `\u2717 ${t('feedback.dismissed')}`
              : `\u2298 ${t('feedback.irrelevant')}`}
          </span>
        )}

        {/* Expand button */}
        <button
          onClick={onToggleExpand}
          aria-expanded={isExpanded}
          aria-controls={`result-detail-${item.id}`}
          aria-label={isExpanded ? t('results.collapseDetails') : t('results.expandDetails')}
          className="flex-shrink-0 text-text-muted text-xs hover:text-text-secondary transition-colors px-1"
        >
          {isExpanded ? '\u2212' : '+'}
        </button>
      </div>

      {/* Secondary row: fallback reason (only when expanded and explanation is absent) */}
      {isExpanded && expandedReason && (
        <div className="mt-1.5 text-xs text-text-secondary ps-[3.75rem]">
          {expandedReason}
        </div>
      )}

      {/* Similar items (collapsed by default, only when expanded) */}
      {isExpanded && (item.similar_count ?? 0) > 0 && (
        <details className="mt-1 ps-[3.75rem] group">
          <summary className="text-[10px] text-text-muted cursor-pointer hover:text-text-secondary select-none list-none flex items-center gap-1">
            <span className="text-[10px] text-text-muted group-open:rotate-90 transition-transform">&#9654;</span>
            {t('results.relatedArticles', { count: item.similar_count })}
          </summary>
          {item.similar_titles && item.similar_titles.length > 0 && (
            <ul className="mt-1 ms-3 space-y-0.5">
              {item.similar_titles.map((title, i) => (
                <li key={i} className="text-[10px] text-text-muted truncate">
                  {title}
                </li>
              ))}
            </ul>
          )}
        </details>
      )}

      {/* Stacked sibling advisories for the same package (only when expanded) */}
      {isExpanded && (item.advisory_stack_count ?? 0) > 0 && (
        <details className="mt-1 ps-[3.75rem] group">
          <summary className="text-[10px] text-amber-400/80 cursor-pointer hover:text-amber-300 select-none list-none flex items-center gap-1">
            <span className="text-[10px] group-open:rotate-90 transition-transform">&#9654;</span>
            {t('results.advisoryStackExpanded', { count: item.advisory_stack_count })}
          </summary>
          {item.advisory_stack_titles && item.advisory_stack_titles.length > 0 && (
            <ul className="mt-1 ms-3 space-y-0.5">
              {item.advisory_stack_titles.map((title, i) => (
                <li key={i} className="text-[10px] text-text-muted truncate">
                  {title}
                </li>
              ))}
            </ul>
          )}
        </details>
      )}
    </div>
  );
});

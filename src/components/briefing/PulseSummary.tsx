import { memo, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';
import { RelativeTimestamp } from './BriefingHelpers';
import type { SourceRelevance, SourceHealthStatus } from '../../types';
import type { BriefingState } from '../../store/types';

interface PulseSummaryProps {
  results: SourceRelevance[];
  sourceHealth: SourceHealthStatus[];
  briefing: BriefingState;
  signalCount: number;
  topCount: number;
}

/**
 * Zone 1: The Pulse — One-sentence intelligence summary.
 * Answers: "What happened since I was last here?"
 */
export const PulseSummary = memo(function PulseSummary({
  results,
  sourceHealth,
  briefing,
  signalCount,
  topCount,
}: PulseSummaryProps) {
  const { t } = useTranslation();

  const [diff, setDiff] = useState<{
    new_items: number;
    new_relevant: number;
    hours_since_last: number;
    has_previous: boolean;
  } | null>(null);

  useEffect(() => {
    cmd('get_session_diff').then(setDiff).catch((e) => console.debug('[PulseSummary] session diff:', e));
  }, []);

  const stats = useMemo(() => {
    const total = results.length;
    const relevant = results.filter(r => r.relevant).length;
    const healthyCount = sourceHealth.filter(s => s.status === 'healthy').length;
    const totalSources = sourceHealth.length;
    const avgScore = relevant > 0
      ? results.filter(r => r.relevant).reduce((sum, r) => sum + r.top_score, 0) / relevant
      : 0;
    return { total, relevant, healthyCount, totalSources, avgScore };
  }, [results, sourceHealth]);

  // Build the summary sentence
  const summary = useMemo(() => {
    const parts: string[] = [];

    if (stats.total === 0) {
      return t('pulse.noData', 'No intelligence gathered yet. Run an analysis to get started.');
    }

    // New items — prefer session diff when available
    if (diff?.has_previous && diff.new_items > 0) {
      parts.push(t('pulse.newSinceLastSession', {
        newItems: diff.new_items,
        newRelevant: diff.new_relevant,
        defaultValue: '{{newItems}} new items since last session. {{newRelevant}} newly relevant.',
      }));
    } else if (diff?.has_previous && diff.new_items === 0) {
      parts.push(t('pulse.noNewItems', 'No new items since last session.'));
    } else {
      parts.push(t('pulse.itemsAnalyzed', {
        count: stats.total,
        relevant: stats.relevant,
        defaultValue: '{{count}} items analyzed, {{relevant}} relevant to you.',
      }));
    }

    // Signals needing attention
    if (signalCount > 0) {
      parts.push(t('pulse.signalsNeedAttention', {
        count: signalCount,
        defaultValue: '{{count}} need your attention.',
      }));
    }

    // Top picks
    if (topCount > 0 && signalCount === 0) {
      parts.push(t('pulse.topPicks', {
        count: topCount,
        defaultValue: '{{count}} top picks.',
      }));
    }

    // Source health warning (only if degraded)
    if (stats.totalSources > 0 && stats.healthyCount < stats.totalSources) {
      parts.push(t('pulse.sourcesPartial', {
        healthy: stats.healthyCount,
        total: stats.totalSources,
        defaultValue: '{{healthy}}/{{total}} sources healthy.',
      }));
    }

    return parts.join(' ');
  }, [stats, signalCount, topCount, diff, t]);

  // Visual state: determine the "mood" of the pulse
  const moodColor = useMemo(() => {
    if (signalCount > 0) return 'text-amber-400';
    if (stats.relevant > 0 && stats.avgScore > 0.6) return 'text-green-400';
    if (stats.total > 0) return 'text-text-secondary';
    return 'text-text-muted';
  }, [signalCount, stats]);

  const moodDot = useMemo(() => {
    if (signalCount > 0) return 'bg-amber-400';
    if (stats.relevant > 0) return 'bg-green-400';
    return 'bg-text-muted/50';
  }, [signalCount, stats.relevant]);

  // AWE voice — subtle wisdom line beneath the pulse
  const aweSummary = useAppStore(s => s.aweSummary);
  const aweWisdomSynthesis = useAppStore(s => s.aweWisdomSynthesis);
  const loadAweSummary = useAppStore(s => s.loadAweSummary);
  useEffect(() => { if (!aweSummary) void loadAweSummary(); }, [aweSummary, loadAweSummary]);

  return (
    <div className="relative px-5 py-4">
      <div className="flex items-center gap-3">
        <div className={`w-2 h-2 rounded-full flex-shrink-0 ${moodDot}`} />
        <div className="flex-1">
          <p className={`text-sm leading-relaxed ${moodColor}`}>
            {summary}
          </p>
          {aweWisdomSynthesis ? (
            <p className="text-[11px] text-accent-gold/80 mt-1 leading-relaxed line-clamp-3">{aweWisdomSynthesis}</p>
          ) : aweSummary?.available === true && aweSummary.decisions > 0 ? (
            <p className="text-[11px] text-accent-gold/70 mt-0.5">
              {aweSummary.feedback_coverage >= 70
                ? t('awe.pulse.compounding', { decisions: aweSummary.decisions, principles: aweSummary.principles })
                : t('awe.pulse.learning', { decisions: aweSummary.decisions, coverage: aweSummary.feedback_coverage })}
            </p>
          ) : null}
        </div>
        {briefing.lastGenerated && (
          <div className="flex-shrink-0 ms-auto">
            <RelativeTimestamp date={briefing.lastGenerated} />
          </div>
        )}
      </div>
    </div>
  );
});

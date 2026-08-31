// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo } from 'react';
import type { SourceRelevance, SourceHealthStatus } from '../types';
import { parseBriefingContent } from '../utils/briefing-parser';
import { isBriefSuppressed } from '../utils/brief-verdicts';
import type { BriefingState } from '../store';

const NO_FILTERED_IDS: ReadonlySet<number> = new Set<number>();

interface LowQualitySource {
  source: string;
  total: number;
  relevant: number;
  ratio: number;
}

interface HealthSummary {
  healthy: number;
  total: number;
  allHealthy: boolean;
}

/**
 * Pure derived computations from briefing-related store state.
 * Extracts useMemo logic from BriefingView to keep the component lean.
 */
export function useBriefingDerived(
  results: SourceRelevance[],
  sourceHealth: SourceHealthStatus[],
  briefing: BriefingState,
  lastBackgroundResultsAt: Date | null,
  /**
   * AD-035: item ids the LATEST briefing filtered (empty when no fresh
   * verdicts bind). Demote-only — these ids lose their attention-card slot
   * but stay in the ordinary feed; `is_critical_alert` items are exempt.
   */
  briefFilteredIds: ReadonlySet<number> = NO_FILTERED_IDS,
) {
  // Intelligence gaps — non-healthy sources
  const gaps = useMemo(
    () => sourceHealth.filter(s => s.status !== 'healthy' && s.gap_message),
    [sourceHealth],
  );

  // Source quality analysis — flag sources with < 5% relevance ratio
  const lowQualitySources = useMemo(() => {
    if (results.length < 10) return [] as LowQualitySource[];
    const bySource: Record<string, { total: number; relevant: number }> = {};
    for (const r of results) {
      const src = r.source_type ?? 'unknown';
      if (!bySource[src]) bySource[src] = { total: 0, relevant: 0 };
      bySource[src].total++;
      if (r.relevant) bySource[src].relevant++;
    }
    return Object.entries(bySource)
      .filter(([, stats]) => stats.total >= 5 && (stats.relevant / stats.total) < 0.05)
      .map(([source, stats]) => ({
        source,
        total: stats.total,
        relevant: stats.relevant,
        ratio: Math.round((stats.relevant / stats.total) * 100),
      }));
  }, [results]);

  // Source health summary for header badge
  const healthSummary = useMemo(() => {
    if (sourceHealth.length === 0) return null;
    const healthy = sourceHealth.filter(s => s.status === 'healthy').length;
    const total = sourceHealth.length;
    return { healthy, total, allHealthy: healthy === total } as HealthSummary;
  }, [sourceHealth]);

  // Parse briefing sections
  const sections = useMemo(() => {
    if (!briefing.content) return [];
    return parseBriefingContent(briefing.content);
  }, [briefing.content]);

  // Detect stale briefing with new items available
  const isStale = useMemo(() => {
    if (!briefing.lastGenerated || !lastBackgroundResultsAt) return false;
    return lastBackgroundResultsAt.getTime() > briefing.lastGenerated.getTime();
  }, [briefing.lastGenerated, lastBackgroundResultsAt]);

  // Critical/alert signal items for action cards.
  //
  // Items with `is_critical_alert === true` render in the persistent
  // `CriticalAlertBanner` at the top of the view — those are
  // intentionally excluded from this card strip so the same item doesn't
  // appear twice (banner AND card) with conflicting score framings.
  const signalItems = useMemo(() => {
    const picked = results
      .filter(r =>
        (r.signal_priority === 'critical' || r.signal_priority === 'alert')
        && !(r.is_critical_alert === true)
        && !isBriefSuppressed(r, briefFilteredIds),
      )
      .slice(0, 3);
    // AD-035 observability: every suppression leaves a trace.
    const suppressed = results.filter(r =>
      (r.signal_priority === 'critical' || r.signal_priority === 'alert')
      && !(r.is_critical_alert === true)
      && isBriefSuppressed(r, briefFilteredIds),
    );
    if (suppressed.length > 0) {
      console.info(
        `[brief-verdicts] ${suppressed.length} attention card(s) demoted by the latest briefing's verdicts`,
        suppressed.map(r => r.id),
      );
    }
    return picked;
  }, [results, briefFilteredIds]);

  // Top picks — sorted by actionability (necessity × relevance) so security
  // CVEs with matched deps always outrank generic awareness items.
  const topItems = useMemo(() => {
    const signalIds = new Set(signalItems.map(s => s.id));
    return results
      .filter(r =>
        r.relevant
        && r.top_score >= 0.5
        && !signalIds.has(r.id)
        && !(r.is_critical_alert === true)
        && !isBriefSuppressed(r, briefFilteredIds),
      )
      .sort((a, b) => {
        const aN = a.score_breakdown?.necessity_score ?? 0;
        const bN = b.score_breakdown?.necessity_score ?? 0;
        const aScore = a.top_score + aN * 0.4;
        const bScore = b.top_score + bN * 0.4;
        return bScore - aScore;
      })
      .slice(0, 8);
  }, [results, signalItems, briefFilteredIds]);

  return { gaps, lowQualitySources, healthSummary, sections, isStale, signalItems, topItems };
}

// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import { recordTrustEvent } from '../../lib/trust-feedback';

const SCORE_TIERS = [
  { max: 25, color: 'text-green-400', bg: 'bg-green-500', labelKey: 'blindspots.score.good' },
  { max: 50, color: 'text-yellow-400', bg: 'bg-yellow-500', labelKey: 'blindspots.score.moderate' },
  { max: 75, color: 'text-orange-400', bg: 'bg-orange-500', labelKey: 'blindspots.score.significant' },
  { max: 100, color: 'text-red-400', bg: 'bg-red-500', labelKey: 'blindspots.score.critical' },
] as const;

const URGENCY_COLORS: Record<Urgency, string> = {
  critical: 'text-red-400',
  high: 'text-orange-400',
  medium: 'text-yellow-400',
  watch: 'text-blue-400',
};

function getScoreTier(score: number) {
  return SCORE_TIERS.find(t => score <= t.max) ?? SCORE_TIERS[3];
}

/** Extract the plain topic/dep name for telemetry. Prefer affected_deps[0]
 * when present (uncovered-dep items) otherwise the title. */
function topicFromItem(item: EvidenceItem): string {
  if (item.affected_deps.length > 0) return item.affected_deps[0]!;
  return item.title;
}

const ScoreBar = memo(function ScoreBar({ score }: { score: number }) {
  const { t } = useTranslation();
  const tier = getScoreTier(score);
  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-5">
      <div className="flex items-baseline gap-3 mb-3">
        <span className={`text-3xl font-semibold tabular-nums ${tier.color}`}>
          {Math.round(score)}
        </span>
        <span className="text-text-muted text-sm">/100</span>
        <span className={`text-sm ${tier.color}`}>{t(tier.labelKey)}</span>
      </div>
      <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${tier.bg}`}
          style={{ width: `${Math.min(score, 100)}%` }}
        />
      </div>
    </div>
  );
});

const UncoveredDepsSection = memo(function UncoveredDepsSection({ items }: { items: EvidenceItem[] }) {
  const { t } = useTranslation();
  const surfacedRef = useRef(new Set<string>());

  useEffect(() => {
    for (const it of items) {
      if (!surfacedRef.current.has(it.id)) {
        surfacedRef.current.add(it.id);
        recordTrustEvent({
          eventType: 'surfaced',
          sourceType: 'gap',
          topic: topicFromItem(it),
          notes: 'blind_spot_uncovered_dep',
        });
      }
    }
  }, [items]);

  if (items.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.uncovered')} ({items.length})
        </h3>
      </div>
      <div className="divide-y divide-border">
        {items.map((it) => (
          <div
            key={it.id}
            className="px-5 py-4 cursor-pointer hover:bg-bg-tertiary/50 transition-colors"
            onClick={() => {
              recordTrustEvent({
                eventType: 'acted_on',
                sourceType: 'gap',
                topic: topicFromItem(it),
                notes: 'blind_spot_dep_click',
              });
            }}
          >
            <div className="flex items-center gap-2 mb-1">
              <span className="text-sm font-medium text-white">{topicFromItem(it)}</span>
              {it.affected_projects.length > 0 && (
                <span className="text-[10px] text-text-muted">
                  {it.affected_projects.length} {it.affected_projects.length === 1 ? 'project' : 'projects'}
                </span>
              )}
              <span className={`ms-auto text-[10px] px-1.5 py-0.5 rounded ${URGENCY_COLORS[it.urgency]}`}>
                {it.urgency}
              </span>
            </div>
            <p className="text-xs text-text-muted">{it.explanation}</p>
          </div>
        ))}
      </div>
    </div>
  );
});

const MissedSignalsSection = memo(function MissedSignalsSection({ items }: { items: EvidenceItem[] }) {
  const { t } = useTranslation();
  const surfacedRef = useRef(new Set<string>());

  useEffect(() => {
    for (const it of items) {
      if (!surfacedRef.current.has(it.id)) {
        surfacedRef.current.add(it.id);
        recordTrustEvent({
          eventType: 'surfaced',
          signalId: it.id,
          sourceType: 'missed_signal',
          topic: it.title,
          notes: 'blind_spot_missed_signal',
        });
      }
    }
  }, [items]);

  if (items.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.missed')} ({items.length})
        </h3>
      </div>
      <div className="divide-y divide-border">
        {items.map((it) => {
          const primaryCitation = it.evidence[0];
          return (
            <div key={it.id} className="px-5 py-4">
              <div className="mb-1">
                {primaryCitation?.url ? (
                  <a
                    href={primaryCitation.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm text-white hover:text-amber-400 transition-colors"
                    onClick={() => {
                      recordTrustEvent({
                        eventType: 'acted_on',
                        signalId: it.id,
                        sourceType: 'missed_signal',
                        topic: it.title,
                        notes: 'blind_spot_click',
                      });
                    }}
                  >
                    {it.title}
                  </a>
                ) : (
                  <span className="text-sm text-white">{it.title}</span>
                )}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-muted">
                <span>{t('blindspots.missed.relevance', { score: it.confidence.value.toFixed(2) })}</span>
                {primaryCitation && (
                  <>
                    <span>·</span>
                    <span>{primaryCitation.source}</span>
                    <span>·</span>
                    <span>
                      {primaryCitation.freshness_days <= 0
                        ? 'today'
                        : `${Math.round(primaryCitation.freshness_days)}d ago`}
                    </span>
                  </>
                )}
              </div>
              {it.explanation && (
                <p className="text-[11px] text-text-secondary mt-1">{it.explanation}</p>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});

const RecommendationsSection = memo(function RecommendationsSection({ items }: { items: EvidenceItem[] }) {
  const { t } = useTranslation();
  if (items.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">{t('blindspots.recommendations')}</h3>
      </div>
      <div className="p-4 space-y-2">
        {items.map((it) => {
          const style =
            it.urgency === 'high' ? 'text-red-400 bg-red-500/10 border-red-500/20'
            : it.urgency === 'medium' ? 'text-yellow-400 bg-yellow-500/10 border-yellow-500/20'
            : 'text-blue-400 bg-blue-500/10 border-blue-500/20';
          return (
            <div key={it.id} className={`px-4 py-3 rounded-lg border ${style}`}>
              <div className="flex items-center gap-2">
                <span className="text-sm">{it.title}</span>
                <span className="ms-auto text-[10px] uppercase font-medium">{it.urgency}</span>
              </div>
              {it.explanation && (
                <p className="text-[11px] text-text-secondary mt-1">{it.explanation}</p>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});

const BlindSpotsView = memo(function BlindSpotsView() {
  const { t } = useTranslation();
  const { report, loading, error } = useAppStore(
    useShallow((s) => ({
      report: s.blindSpotReport,
      loading: s.blindSpotsLoading,
      error: s.blindSpotsError,
    })),
  );
  const loadBlindSpots = useAppStore((s) => s.loadBlindSpots);

  useEffect(() => {
    loadBlindSpots();
  }, [loadBlindSpots]);

  const handleRetry = useCallback(() => {
    loadBlindSpots();
  }, [loadBlindSpots]);

  // Bucket items by the underlying legacy concept (preserved via id prefix
  // in the Rust materializer) so this view's UX remains unchanged while
  // the backend emits a single canonical feed.
  const buckets = useMemo(() => {
    const uncovered: EvidenceItem[] = [];
    const missed: EvidenceItem[] = [];
    const recommendations: EvidenceItem[] = [];
    for (const it of report?.items ?? []) {
      if (it.id.startsWith('bs_uncov_') || it.id.startsWith('bs_stale_')) {
        uncovered.push(it);
      } else if (it.id.startsWith('bs_missed_')) {
        missed.push(it);
      } else if (it.id.startsWith('bs_rec_')) {
        recommendations.push(it);
      }
    }
    return { uncovered, missed, recommendations };
  }, [report]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-text-secondary text-sm">
        {t('blindspots.loading')}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20 gap-3">
        <p className="text-red-400 text-sm">{error}</p>
        <button onClick={handleRetry} className="text-sm text-text-secondary hover:text-white transition-colors">
          {t('action.retry')}
        </button>
      </div>
    );
  }

  if (!report) {
    return (
      <div className="flex items-center justify-center py-20 text-text-muted text-sm">
        {t('blindspots.empty')}
      </div>
    );
  }

  const score = report.score ?? 0;
  const hasContent =
    buckets.uncovered.length > 0 ||
    buckets.missed.length > 0 ||
    buckets.recommendations.length > 0;

  return (
    <div className="space-y-4 pb-8">
      <div className="mb-2">
        <h2 className="text-lg font-semibold text-white">{t('blindspots.title')}</h2>
        <p className="text-sm text-text-muted">{t('blindspots.subtitle')}</p>
      </div>

      <ScoreBar score={score} />

      {!hasContent ? (
        <div className="bg-bg-secondary rounded-lg border border-border px-5 py-8 text-center">
          <p className="text-sm text-text-muted">{t('blindspots.empty')}</p>
        </div>
      ) : (
        <>
          <UncoveredDepsSection items={buckets.uncovered} />
          <MissedSignalsSection items={buckets.missed} />
          <RecommendationsSection items={buckets.recommendations} />
        </>
      )}
    </div>
  );
});

export default BlindSpotsView;

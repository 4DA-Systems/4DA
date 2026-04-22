// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useRef, useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { ArticleReader } from '../ArticleReader';

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

function topicFromItem(item: EvidenceItem): string {
  if (item.affected_deps.length > 0) return item.affected_deps[0]!;
  return item.title;
}

function extractItemId(evidenceId: string): number | null {
  const match = evidenceId.match(/bs_missed_(\d+)/);
  return match ? parseInt(match[1]!, 10) : null;
}

// ---------------------------------------------------------------------------
// Score Bar
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Missed Signal Row (used inside expanded coverage gaps AND standalone)
// ---------------------------------------------------------------------------

const MissedSignalRow = memo(function MissedSignalRow({
  item,
  onDismiss,
}: {
  item: EvidenceItem;
  onDismiss?: (id: string) => void;
}) {
  const primaryCitation = item.evidence[0];
  const numericId = extractItemId(item.id);
  const freshness = primaryCitation && primaryCitation.freshness_days > 0
    ? `${Math.round(primaryCitation.freshness_days)}d ago`
    : 'today';

  return (
    <div className="px-5 py-3 hover:bg-bg-tertiary/30 transition-colors group">
      <div className="flex items-start gap-3">
        <div className="flex-1 min-w-0">
          <div className="mb-1">
            {numericId != null ? (
              <div className="inline">
                <ArticleReader
                  itemId={numericId}
                  url={primaryCitation?.url ?? undefined}
                  contentType={primaryCitation?.source}
                />
              </div>
            ) : primaryCitation?.url ? (
              <a
                href={primaryCitation.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-white hover:text-amber-400 transition-colors"
                onClick={() => {
                  recordTrustEvent({
                    eventType: 'acted_on',
                    signalId: item.id,
                    sourceType: 'missed_signal',
                    topic: item.title,
                    notes: 'blind_spot_click',
                  });
                }}
              >
                {item.title}
              </a>
            ) : (
              <span className="text-sm text-white">{item.title}</span>
            )}
          </div>
          <div className="flex items-center gap-2 text-xs text-text-muted">
            {primaryCitation && (
              <>
                <span>{primaryCitation.source}</span>
                <span>·</span>
                <span>{freshness}</span>
              </>
            )}
            {item.explanation && (
              <>
                <span>·</span>
                <span className="truncate">{item.explanation}</span>
              </>
            )}
          </div>
        </div>
        {onDismiss && (
          <button
            onClick={() => onDismiss(item.id)}
            className="text-xs text-text-muted hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all shrink-0 px-2 py-1 rounded hover:bg-red-500/10"
            title="Not relevant"
          >
            ✕
          </button>
        )}
      </div>
    </div>
  );
});

// ---------------------------------------------------------------------------
// Expandable Coverage Gap
// ---------------------------------------------------------------------------

const CoverageGapCard = memo(function CoverageGapCard({
  gap,
  signals,
  onDismissSignal,
}: {
  gap: EvidenceItem;
  signals: EvidenceItem[];
  onDismissSignal: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const depName = topicFromItem(gap);

  const handleToggle = useCallback(() => {
    setExpanded(prev => !prev);
    if (!expanded) {
      recordTrustEvent({
        eventType: 'acted_on',
        sourceType: 'gap',
        topic: depName,
        notes: 'blind_spot_gap_expand',
      });
    }
  }, [expanded, depName]);

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        onClick={handleToggle}
        className="w-full px-5 py-4 flex items-center gap-3 hover:bg-bg-tertiary/30 transition-colors text-left"
      >
        <span className={`text-[10px] transition-transform ${expanded ? 'rotate-90' : ''}`}>▶</span>
        <span className="text-sm font-medium text-white flex-1">{gap.title}</span>
        {gap.affected_projects.length > 0 && (
          <span className="text-[10px] text-text-muted">
            {gap.affected_projects.length} {gap.affected_projects.length === 1 ? 'project' : 'projects'}
          </span>
        )}
        <span className={`text-[10px] px-1.5 py-0.5 rounded ${URGENCY_COLORS[gap.urgency]}`}>
          {gap.urgency}
        </span>
      </button>
      {expanded && (
        <div className="bg-bg-tertiary/20 border-t border-border">
          {signals.length > 0 ? (
            <div className="divide-y divide-border/50">
              {signals.map(s => (
                <MissedSignalRow key={s.id} item={s} onDismiss={onDismissSignal} />
              ))}
            </div>
          ) : (
            <p className="px-5 py-3 text-xs text-text-muted">
              {gap.explanation}
            </p>
          )}
        </div>
      )}
    </div>
  );
});

// ---------------------------------------------------------------------------
// Coverage Gaps Section
// ---------------------------------------------------------------------------

const CoverageGapsSection = memo(function CoverageGapsSection({
  gaps,
  allMissed,
  onDismissSignal,
}: {
  gaps: EvidenceItem[];
  allMissed: EvidenceItem[];
  onDismissSignal: (id: string) => void;
}) {
  const { t } = useTranslation();
  const surfacedRef = useRef(new Set<string>());

  useEffect(() => {
    for (const it of gaps) {
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
  }, [gaps]);

  const signalsByDep = useMemo(() => {
    const map = new Map<string, EvidenceItem[]>();
    for (const signal of allMissed) {
      for (const dep of signal.affected_deps) {
        const list = map.get(dep) ?? [];
        list.push(signal);
        map.set(dep, list);
      }
    }
    return map;
  }, [allMissed]);

  if (gaps.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.uncovered')} ({gaps.length})
        </h3>
        <p className="text-[11px] text-text-muted mt-0.5">Click to see the signals you missed</p>
      </div>
      <div>
        {gaps.map(gap => {
          const dep = topicFromItem(gap);
          const signals = signalsByDep.get(dep) ?? [];
          return (
            <CoverageGapCard
              key={gap.id}
              gap={gap}
              signals={signals}
              onDismissSignal={onDismissSignal}
            />
          );
        })}
      </div>
    </div>
  );
});

// ---------------------------------------------------------------------------
// Unlinked Missed Signals (not associated with any coverage gap)
// ---------------------------------------------------------------------------

const UnlinkedMissedSection = memo(function UnlinkedMissedSection({
  items,
  onDismiss,
}: {
  items: EvidenceItem[];
  onDismiss: (id: string) => void;
}) {
  const { t } = useTranslation();

  if (items.length === 0) return null;
  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.missed')} ({items.length})
        </h3>
      </div>
      <div className="divide-y divide-border">
        {items.map(it => (
          <MissedSignalRow key={it.id} item={it} onDismiss={onDismiss} />
        ))}
      </div>
    </div>
  );
});

// ---------------------------------------------------------------------------
// Main View
// ---------------------------------------------------------------------------

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
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadBlindSpots();
  }, [loadBlindSpots]);

  const handleRetry = useCallback(() => {
    loadBlindSpots();
  }, [loadBlindSpots]);

  const handleDismiss = useCallback((id: string) => {
    setDismissed(prev => new Set(prev).add(id));
    recordTrustEvent({
      eventType: 'dismissed',
      signalId: id,
      sourceType: 'missed_signal',
      notes: 'blind_spot_not_relevant',
    });
  }, []);

  const buckets = useMemo(() => {
    const uncovered: EvidenceItem[] = [];
    const missed: EvidenceItem[] = [];
    for (const it of report?.items ?? []) {
      if (dismissed.has(it.id)) continue;
      if (it.id.startsWith('bs_uncov_') || it.id.startsWith('bs_stale_')) {
        uncovered.push(it);
      } else if (it.id.startsWith('bs_missed_')) {
        missed.push(it);
      }
    }
    return { uncovered, missed };
  }, [report, dismissed]);

  const linkedDeps = useMemo(() => {
    const deps = new Set<string>();
    for (const gap of buckets.uncovered) {
      for (const dep of gap.affected_deps) deps.add(dep);
    }
    return deps;
  }, [buckets.uncovered]);

  const unlinkedMissed = useMemo(() => {
    return buckets.missed.filter(
      m => m.affected_deps.length === 0 || !m.affected_deps.some(d => linkedDeps.has(d)),
    );
  }, [buckets.missed, linkedDeps]);

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
  const totalActive = buckets.uncovered.length + buckets.missed.length;
  const dismissedCount = dismissed.size;

  const scoreContext = dismissedCount > 0
    ? `${dismissedCount} item${dismissedCount === 1 ? '' : 's'} reviewed this session. ${totalActive} remaining.`
    : totalActive > 0
      ? `${buckets.missed.length} missed signal${buckets.missed.length === 1 ? '' : 's'} and ${buckets.uncovered.length} coverage gap${buckets.uncovered.length === 1 ? '' : 's'}. Expand gaps to see what you missed.`
      : 'Your stack coverage is excellent.';

  return (
    <div className="space-y-4 pb-8">
      <div className="mb-2">
        <h2 className="text-lg font-semibold text-white">{t('blindspots.title')}</h2>
        <p className="text-sm text-text-muted">{t('blindspots.subtitle')}</p>
      </div>

      <ScoreBar score={score} />
      <p className="text-xs text-text-muted px-1 -mt-2">{scoreContext}</p>

      {totalActive === 0 ? (
        <div className="bg-bg-secondary rounded-lg border border-border px-5 py-8 text-center">
          <p className="text-sm text-text-muted">{t('blindspots.empty')}</p>
        </div>
      ) : (
        <>
          <CoverageGapsSection
            gaps={buckets.uncovered}
            allMissed={buckets.missed}
            onDismissSignal={handleDismiss}
          />
          <UnlinkedMissedSection items={unlinkedMissed} onDismiss={handleDismiss} />
        </>
      )}
    </div>
  );
});

export default BlindSpotsView;

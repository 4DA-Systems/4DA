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

const UNLINKED_LIMIT = 5;

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

function signalMatchesDep(signal: EvidenceItem, depName: string): boolean {
  const lower = depName.toLowerCase();
  return signal.affected_deps.some(d => d.toLowerCase() === lower)
    || signal.title.toLowerCase().includes(lower);
}

// --- Score Bar ---------------------------------------------------------------

const ScoreBar = memo(function ScoreBar({ score }: { score: number }) {
  const { t } = useTranslation();
  const tier = getScoreTier(score);
  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-5">
      <div className="flex items-baseline gap-3 mb-3">
        <span className={`text-3xl font-semibold tabular-nums ${tier.color}`}>{Math.round(score)}</span>
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

// --- Missed Signal Row — title is the primary visible element ----------------

const MissedSignalRow = memo(function MissedSignalRow({
  item, onDismiss,
}: {
  item: EvidenceItem;
  onDismiss?: (id: string) => void;
}) {
  const cite = item.evidence[0];
  const numericId = extractItemId(item.id);
  const freshness = cite && cite.freshness_days > 0
    ? `${Math.round(cite.freshness_days)}d ago` : 'today';

  return (
    <div className="px-5 py-3 hover:bg-bg-tertiary/30 transition-colors group">
      <div className="flex items-start gap-3">
        <div className="flex-1 min-w-0">
          <div className="mb-1 flex items-center gap-2">
            {cite?.url ? (
              <a
                href={cite.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-white hover:text-amber-400 transition-colors leading-snug"
                onClick={() => recordTrustEvent({
                  eventType: 'acted_on', signalId: item.id,
                  sourceType: 'missed_signal', topic: item.title, notes: 'blind_spot_click',
                })}
              >
                {item.title}
              </a>
            ) : (
              <span className="text-sm text-white leading-snug">{item.title}</span>
            )}
            <span className={`text-[10px] px-1.5 py-0.5 rounded shrink-0 ${URGENCY_COLORS[item.urgency]}`}>
              {item.urgency}
            </span>
          </div>
          <div className="flex items-center gap-2 text-xs text-text-muted">
            {cite && (<><span>{cite.source}</span><span>·</span><span>{freshness}</span></>)}
            {item.explanation && (<><span>·</span><span className="truncate">{item.explanation}</span></>)}
          </div>
          {numericId != null && (
            <div className="mt-1.5">
              <ArticleReader itemId={numericId} url={cite?.url ?? undefined} contentType={cite?.source} />
            </div>
          )}
        </div>
        {onDismiss && (
          <button
            onClick={() => onDismiss(item.id)}
            className="text-xs text-text-muted hover:text-red-400 opacity-60 group-hover:opacity-100 transition-all shrink-0 px-2 py-1 rounded hover:bg-red-500/10"
            title="Not relevant"
          >
            ✕
          </button>
        )}
      </div>
    </div>
  );
});

// --- Expandable Coverage Gap -------------------------------------------------

const CoverageGapCard = memo(function CoverageGapCard({
  gap, signals, onDismissSignal,
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
      recordTrustEvent({ eventType: 'acted_on', sourceType: 'gap', topic: depName, notes: 'blind_spot_gap_expand' });
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
        {signals.length > 0 && (
          <span className="text-[10px] text-text-muted">
            {signals.length} signal{signals.length === 1 ? '' : 's'}
          </span>
        )}
        {gap.affected_projects.length > 0 && (
          <span className="text-[10px] text-text-muted">
            {gap.affected_projects.length} {gap.affected_projects.length === 1 ? 'project' : 'projects'}
          </span>
        )}
        <span className={`text-[10px] px-1.5 py-0.5 rounded ${URGENCY_COLORS[gap.urgency]}`}>{gap.urgency}</span>
      </button>
      {expanded && (
        <div className="bg-bg-tertiary/20 border-t border-border">
          {signals.length > 0 ? (
            <div className="divide-y divide-border/50">
              {signals.map(s => <MissedSignalRow key={s.id} item={s} onDismiss={onDismissSignal} />)}
            </div>
          ) : (
            <div className="px-5 py-4">
              <p className="text-xs text-text-muted mb-2">{gap.explanation}</p>
              <p className="text-[11px] text-text-muted italic">
                No matching signals found — potential gap in source coverage for{' '}
                <span className="text-white font-medium">{depName}</span>.
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
});

// --- Coverage Gaps Section ---------------------------------------------------

const CoverageGapsSection = memo(function CoverageGapsSection({
  gaps, allMissed, onDismissSignal,
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
        recordTrustEvent({ eventType: 'surfaced', sourceType: 'gap', topic: topicFromItem(it), notes: 'blind_spot_uncovered_dep' });
      }
    }
  }, [gaps]);

  const signalsByGapId = useMemo(() => {
    const map = new Map<string, EvidenceItem[]>();
    for (const gap of gaps) {
      const dep = topicFromItem(gap);
      map.set(gap.id, allMissed.filter(s => signalMatchesDep(s, dep)));
    }
    return map;
  }, [gaps, allMissed]);

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
        {gaps.map(gap => (
          <CoverageGapCard
            key={gap.id} gap={gap}
            signals={signalsByGapId.get(gap.id) ?? []}
            onDismissSignal={onDismissSignal}
          />
        ))}
      </div>
    </div>
  );
});

// --- Unlinked Missed Signals — limited with show-all toggle ------------------

const UnlinkedMissedSection = memo(function UnlinkedMissedSection({
  items, onDismiss,
}: {
  items: EvidenceItem[];
  onDismiss: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [showAll, setShowAll] = useState(false);
  const visible = showAll ? items : items.slice(0, UNLINKED_LIMIT);
  const hiddenCount = items.length - UNLINKED_LIMIT;

  if (items.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">
          {t('blindspots.missed')} ({items.length})
        </h3>
        <p className="text-[11px] text-text-muted mt-0.5">Signals not tied to a specific coverage gap</p>
      </div>
      <div className="divide-y divide-border">
        {visible.map(it => <MissedSignalRow key={it.id} item={it} onDismiss={onDismiss} />)}
      </div>
      {!showAll && hiddenCount > 0 && (
        <button
          onClick={() => setShowAll(true)}
          className="w-full px-5 py-3 text-xs text-text-muted hover:text-white hover:bg-bg-tertiary/30 transition-colors border-t border-border"
        >
          Show {hiddenCount} more signal{hiddenCount === 1 ? '' : 's'}
        </button>
      )}
    </div>
  );
});

// --- Main View ---------------------------------------------------------------

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

  useEffect(() => { loadBlindSpots(); }, [loadBlindSpots]);

  const handleRetry = useCallback(() => { loadBlindSpots(); }, [loadBlindSpots]);

  const handleDismiss = useCallback((id: string) => {
    setDismissed(prev => new Set(prev).add(id));
    recordTrustEvent({ eventType: 'dismissed', signalId: id, sourceType: 'missed_signal', notes: 'blind_spot_not_relevant' });
  }, []);

  const buckets = useMemo(() => {
    const uncovered: EvidenceItem[] = [];
    const missed: EvidenceItem[] = [];
    for (const it of report?.items ?? []) {
      if (dismissed.has(it.id)) continue;
      if (it.id.startsWith('bs_uncov_') || it.id.startsWith('bs_stale_')) uncovered.push(it);
      else if (it.id.startsWith('bs_missed_')) missed.push(it);
    }
    return { uncovered, missed };
  }, [report, dismissed]);

  const linkedSignalIds = useMemo(() => {
    const ids = new Set<string>();
    for (const gap of buckets.uncovered) {
      const dep = topicFromItem(gap);
      for (const s of buckets.missed) { if (signalMatchesDep(s, dep)) ids.add(s.id); }
    }
    return ids;
  }, [buckets.uncovered, buckets.missed]);

  const unlinkedMissed = useMemo(
    () => buckets.missed.filter(m => !linkedSignalIds.has(m.id)),
    [buckets.missed, linkedSignalIds],
  );

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
  const dc = dismissed.size;
  const gapN = buckets.uncovered.length;
  const sigN = buckets.missed.length;

  const scoreContext = dc > 0
    ? `${dc} reviewed this session. ${totalActive} active: ${gapN} gap${gapN === 1 ? '' : 's'}, ${sigN} signal${sigN === 1 ? '' : 's'}.`
    : totalActive > 0
      ? `${gapN} coverage gap${gapN === 1 ? '' : 's'} in your stack and ${sigN} signal${sigN === 1 ? '' : 's'} you haven't engaged with.`
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
          <CoverageGapsSection gaps={buckets.uncovered} allMissed={buckets.missed} onDismissSignal={handleDismiss} />
          <UnlinkedMissedSection items={unlinkedMissed} onDismiss={handleDismiss} />
        </>
      )}
    </div>
  );
});

export default BlindSpotsView;

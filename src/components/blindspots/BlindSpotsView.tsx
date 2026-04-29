// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useRef, useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { useColdStartGate } from '../../hooks/use-cold-start-gate';
import {
  type DepRow, type DepStatus, URGENCY_ORDER,
  getScoreTier, depFromItem, signalMatchesDep,
} from './types';
import { TierSection, EmergingSignals, CoveredSection } from './StackCoverageMap';

// ============================================================================
// Score Bar
// ============================================================================

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

// ============================================================================
// Main View — data shaping + layout
// ============================================================================

const BlindSpotsView = memo(function BlindSpotsView() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
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

  const { depRows, unmatchedSignals } = useMemo(() => {
    const items = (report?.items ?? []).filter(it => !dismissed.has(it.id));

    const gaps = items.filter(it => it.id.startsWith('bs_uncov_') || it.id.startsWith('bs_stale_'));
    const missed = items.filter(it => it.id.startsWith('bs_missed_'));

    const depMap = new Map<string, DepRow>();

    for (const gap of gaps) {
      const dep = depFromItem(gap);
      if (!dep) continue;
      const key = dep.toLowerCase();
      if (!depMap.has(key)) {
        depMap.set(key, {
          name: dep, status: 'blind_spot', urgency: gap.urgency,
          gap, signals: [], projects: gap.affected_projects,
        });
      }
    }

    const matchedSignalIds = new Set<string>();

    for (const signal of missed) {
      for (const [, row] of depMap) {
        if (signalMatchesDep(signal, row.name)) {
          row.signals.push(signal);
          matchedSignalIds.add(signal.id);
          break;
        }
      }
    }

    for (const signal of missed) {
      if (matchedSignalIds.has(signal.id)) continue;
      const dep = depFromItem(signal);
      if (!dep) continue;
      const key = dep.toLowerCase();
      if (!depMap.has(key)) {
        depMap.set(key, {
          name: dep, status: 'falling_behind', urgency: signal.urgency,
          gap: null, signals: [], projects: [],
        });
      }
      depMap.get(key)!.signals.push(signal);
      matchedSignalIds.add(signal.id);
    }

    for (const row of depMap.values()) {
      if (row.gap && (row.gap.urgency === 'critical' || row.gap.urgency === 'high')) {
        row.status = 'blind_spot';
      } else if (row.gap || row.signals.length >= 3) {
        row.status = row.signals.length > 0 ? 'blind_spot' : 'falling_behind';
      } else if (row.signals.length > 0) {
        row.status = 'falling_behind';
      } else {
        row.status = 'well_covered';
      }
      row.signals.sort((a, b) => URGENCY_ORDER[a.urgency] - URGENCY_ORDER[b.urgency]);
    }

    const statusOrder: Record<DepStatus, number> = { blind_spot: 0, falling_behind: 1, well_covered: 2 };
    const rows = Array.from(depMap.values()).sort((a, b) =>
      statusOrder[a.status] - statusOrder[b.status]
      || URGENCY_ORDER[a.urgency] - URGENCY_ORDER[b.urgency]
    );

    const unmatched = missed.filter(m => !matchedSignalIds.has(m.id));
    return { depRows: rows, unmatchedSignals: unmatched };
  }, [report, dismissed]);

  const surfacedRef = useRef(new Set<string>());
  useEffect(() => {
    for (const row of depRows) {
      if (row.status !== 'well_covered' && !surfacedRef.current.has(row.name)) {
        surfacedRef.current.add(row.name);
        recordTrustEvent({
          eventType: 'surfaced', sourceType: 'gap',
          topic: row.name, notes: `stack_map_${row.status}`,
        });
      }
    }
  }, [depRows]);

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
    // Intelligence Doctrine Rule 6: silent until data arrives
    if (isColdStart) return null;
    return (
      <div className="flex items-center justify-center py-20 text-text-muted text-sm">
        {t('blindspots.empty')}
      </div>
    );
  }

  const score = report.score ?? 0;
  const stackDeps = depRows.filter(d => d.status === 'blind_spot');
  const ecosystemDeps = depRows.filter(d => d.status === 'falling_behind');
  const coveredDeps = depRows.filter(d => d.status === 'well_covered');
  const problemCount = stackDeps.length + ecosystemDeps.length;

  const scoreContext = problemCount === 0
    ? t('blindspots.scoreContext.excellent')
    : t('blindspots.scoreContext.summary', {
        uncoveredText: stackDeps.length > 0 ? t('blindspots.tier.stackSubtitle', { count: stackDeps.length }) : '',
        separator: stackDeps.length > 0 && ecosystemDeps.length > 0 ? ', ' : '',
        driftingText: ecosystemDeps.length > 0 ? t('blindspots.tier.ecosystemSubtitle', { count: ecosystemDeps.length }) : '',
        total: depRows.length,
      });

  const hasContent = stackDeps.length > 0 || ecosystemDeps.length > 0 || unmatchedSignals.length > 0;

  return (
    <div className="space-y-4 pb-8">
      <div className="mb-2">
        <h2 className="text-lg font-semibold text-white">{t('blindspots.title')}</h2>
        <p className="text-sm text-text-muted">{t('blindspots.subtitle')}</p>
      </div>
      <ScoreBar score={score} />
      <p className="text-xs text-text-muted px-1 -mt-2">{scoreContext}</p>
      {!hasContent && coveredDeps.length === 0 ? (
        <div className="bg-bg-secondary rounded-lg border border-border px-5 py-8 text-center">
          <p className="text-sm text-text-muted">{t('blindspots.empty')}</p>
        </div>
      ) : (
        <>
          {(stackDeps.length > 0 || problemCount === 0) && (
            <TierSection
              dotColor="#EF4444"
              borderColor="rgba(239, 68, 68, 0.2)"
              title={t('blindspots.tier.stack')}
              subtitle={t('blindspots.tier.stackSubtitle', { count: stackDeps.length })}
              badgeText={t('blindspots.tier.needsAttention')}
              badgeColor="#EF4444"
              depRows={stackDeps}
              onDismissSignal={handleDismiss}
              emptyText={t('blindspots.tier.stackEmpty')}
            />
          )}
          {(ecosystemDeps.length > 0 || problemCount === 0) && (
            <TierSection
              dotColor="#F59E0B"
              borderColor="rgba(245, 158, 11, 0.15)"
              title={t('blindspots.tier.ecosystem')}
              subtitle={t('blindspots.tier.ecosystemSubtitle', { count: ecosystemDeps.length })}
              badgeText={t('blindspots.tier.drifting')}
              badgeColor="#F59E0B"
              depRows={ecosystemDeps}
              onDismissSignal={handleDismiss}
              emptyText={t('blindspots.tier.ecosystemEmpty')}
            />
          )}
          <EmergingSignals items={unmatchedSignals} onDismiss={handleDismiss} />
          <CoveredSection depRows={coveredDeps} onDismissSignal={handleDismiss} />
        </>
      )}
    </div>
  );
});

export default BlindSpotsView;

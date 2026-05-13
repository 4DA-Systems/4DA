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

const DISMISS_STORAGE_KEY = 'blindspots_dismissed';
const DISMISS_TTL_MS = 7 * 24 * 60 * 60 * 1000;

function loadPersistedDismissals(): Set<string> {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw) as Array<{ id: string; ts: number }>;
    const now = Date.now();
    const valid = parsed.filter(e => now - e.ts < DISMISS_TTL_MS);
    if (valid.length !== parsed.length) {
      localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(valid));
    }
    return new Set(valid.map(e => e.id));
  } catch { return new Set(); }
}

function persistDismissal(id: string) {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    const parsed: Array<{ id: string; ts: number }> = raw ? JSON.parse(raw) : [];
    parsed.push({ id, ts: Date.now() });
    localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed));
  } catch { /* non-fatal */ }
}

function removeDismissal(id: string) {
  try {
    const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
    if (!raw) return;
    const parsed: Array<{ id: string; ts: number }> = JSON.parse(raw);
    localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed.filter(e => e.id !== id)));
  } catch { /* non-fatal */ }
}

// ============================================================================
// Score Bar
// ============================================================================

const ScoreBar = memo(function ScoreBar({ score }: { score: number }) {
  const { t } = useTranslation();

  if (score < 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-5">
        <div className="flex items-baseline gap-3 mb-3">
          <span className="text-lg font-medium text-text-muted">{t('blindspots.score.building')}</span>
        </div>
        <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
          <div className="h-full rounded-full bg-border w-1/4 animate-pulse" />
        </div>
      </div>
    );
  }

  const tier = getScoreTier(score);
  const coverage = Math.round(Math.max(0, 100 - score));
  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-5">
      <div className="flex items-baseline gap-3 mb-3">
        <span className={`text-3xl font-semibold tabular-nums ${tier.color}`}>{coverage}</span>
        <span className="text-text-muted text-sm">/100</span>
        <span className={`text-sm ${tier.color}`}>{t(tier.labelKey)}</span>
      </div>
      <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${tier.bg}`}
          style={{ width: `${coverage}%` }}
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
  const [dismissed, setDismissed] = useState<Set<string>>(loadPersistedDismissals);
  const [lastDismissed, setLastDismissed] = useState<string | null>(null);
  const undoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => { void loadBlindSpots(); }, [loadBlindSpots]);

  const handleRetry = useCallback(() => { void loadBlindSpots(); }, [loadBlindSpots]);

  const handleDismiss = useCallback((id: string) => {
    setDismissed(prev => new Set(prev).add(id));
    persistDismissal(id);
    setLastDismissed(id);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
    undoTimerRef.current = setTimeout(() => setLastDismissed(null), 8000);
    recordTrustEvent({ eventType: 'dismissed', signalId: id, sourceType: 'missed_signal', notes: 'blind_spot_not_relevant' });
  }, []);

  const handleUndo = useCallback(() => {
    if (!lastDismissed) return;
    setDismissed(prev => {
      const next = new Set(prev);
      next.delete(lastDismissed);
      return next;
    });
    removeDismissal(lastDismissed);
    setLastDismissed(null);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
  }, [lastDismissed]);

  const { depRows, unmatchedSignals, recommendations } = useMemo(() => {
    const items = (report?.items ?? []).filter(it => !dismissed.has(it.id));

    const gaps = items.filter(it => it.id.startsWith('bs_uncov_') || it.id.startsWith('bs_stale_'));
    const missed = items.filter(it => it.id.startsWith('bs_missed_') || it.id.startsWith('llm-bs-'));
    const recs = items.filter(it => it.id.startsWith('bs_rec_'));

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
    return { depRows: rows, unmatchedSignals: unmatched, recommendations: recs };
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
    const isTimeoutError = /timed?\s*out|deadline/i.test(error);
    return (
      <div className="space-y-4">
        <div className="mb-2">
          <h2 className="text-lg font-semibold text-white">{t('blindspots.title')}</h2>
          <p className="text-sm text-text-muted">{t('blindspots.subtitle')}</p>
        </div>
        <div className="bg-bg-secondary rounded-lg border border-red-500/20 px-5 py-5">
          <div className="flex items-start gap-4">
            <div className="w-9 h-9 rounded-full bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
              <span className="text-red-300 text-sm">!</span>
            </div>
            <div className="min-w-0 flex-1 space-y-3">
              <div className="space-y-1">
                <h3 className="text-sm font-medium text-white">{t('blindspots.error.title')}</h3>
                <p className="text-sm text-text-muted">
                  {isTimeoutError ? t('blindspots.error.timeout') : t('blindspots.error.subtitle')}
                </p>
              </div>
              <p className="text-xs text-red-300">{error}</p>
              <button
                onClick={handleRetry}
                className="px-3 py-1.5 text-sm text-white bg-bg-tertiary border border-border rounded-lg hover:border-red-500/30 hover:text-red-300 transition-colors"
              >
                {t('action.retry')}
              </button>
            </div>
          </div>
        </div>
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
  const totalTracked = report.total_tracked ?? depRows.length;
  const stackDeps = depRows.filter(d => d.status === 'blind_spot');
  const ecosystemDeps = depRows.filter(d => d.status === 'falling_behind');
  const coveredDeps = depRows.filter(d => d.status === 'well_covered');

  const hasProblems = stackDeps.length > 0 || ecosystemDeps.length > 0;
  const hasContent = hasProblems || unmatchedSignals.length > 0;

  return (
    <div className="space-y-4 pb-8">
      <div className="mb-2">
        <h2 className="text-lg font-semibold text-white">{t('blindspots.title')}</h2>
        <p className="text-sm text-text-muted">{t('blindspots.subtitle')}</p>
      </div>
      <ScoreBar score={score} />
      {hasContent && (
        <div className="flex items-center gap-4 px-4 py-2.5 rounded-lg bg-bg-secondary border border-border -mt-1">
          <div className="flex items-center gap-3 text-xs">
            {stackDeps.length > 0 && (
              <span className="inline-flex items-center gap-1.5 text-red-400 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
                {stackDeps.length} {t('blindspots.tier.needsAttention').toLowerCase()}
              </span>
            )}
            {ecosystemDeps.length > 0 && (
              <span className="inline-flex items-center gap-1.5 text-yellow-400 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-yellow-400" />
                {ecosystemDeps.length} {t('blindspots.tier.drifting').toLowerCase()}
              </span>
            )}
            {unmatchedSignals.length > 0 && (
              <span className="inline-flex items-center gap-1.5 text-blue-400 font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-blue-400" />
                {unmatchedSignals.length} {t('blindspots.emerging.trending').toLowerCase()}
              </span>
            )}
          </div>
          <span className="ms-auto text-xs text-text-muted tabular-nums">
            {t('blindspots.stats.tracked', { count: totalTracked })}
          </span>
        </div>
      )}
      {lastDismissed !== null && (
        <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20 animate-in fade-in">
          <span className="text-xs text-amber-400">{t('blindspots.dismissed')}</span>
          <button
            type="button"
            onClick={handleUndo}
            className="text-xs font-medium text-amber-400 hover:text-white underline-offset-2 hover:underline transition-colors"
          >
            {t('blindspots.action.undo')}
          </button>
        </div>
      )}
      {!hasContent ? (
        score < 0 ? (
          <div className="bg-bg-secondary rounded-lg border border-border px-5 py-8 text-center">
            <p className="text-sm text-text-muted">{t('blindspots.scoreContext.building')}</p>
          </div>
        ) : (
          <div className="bg-bg-secondary rounded-lg border border-emerald-500/20 px-5 py-6">
            <div className="flex items-start gap-4">
              <div className="w-9 h-9 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center shrink-0">
                <span className="text-emerald-400 text-sm">&#10003;</span>
              </div>
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-medium text-white">{t('blindspots.scoreContext.excellent')}</h3>
                {recommendations.length > 0 && (
                  <p className="text-xs text-text-muted mt-1">{recommendations[0]!.explanation}</p>
                )}
              </div>
            </div>
          </div>
        )
      ) : (
        <>
          {stackDeps.length > 0 && (
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
          {ecosystemDeps.length > 0 && (
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
          {recommendations.length > 0 && (
            <div className="space-y-1.5">
              {recommendations.map(rec => (
                <div key={rec.id} className="px-4 py-2.5 bg-bg-secondary rounded-lg border border-border">
                  <p className="text-xs text-text-secondary">{rec.title}</p>
                  <p className="text-[11px] text-text-muted mt-0.5">{rec.explanation}</p>
                </div>
              ))}
            </div>
          )}
        </>
      )}
      <CoveredSection depRows={coveredDeps} onDismissSignal={handleDismiss} />
    </div>
  );
});

export default BlindSpotsView;

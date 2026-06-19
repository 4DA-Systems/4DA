// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useRef, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';

import { useAppStore } from '../../store';
import { cmd } from '../../lib/commands';
import { recordTrustEvent } from '../../lib/trust-feedback';
import { useColdStartGate } from '../../hooks/use-cold-start-gate';
import { useBlindSpotsData } from '../../hooks/use-blind-spots-data';

import { BlindSpotsPaywall } from './BlindSpotsPaywall';
import { loadPersistedDismissals, persistDismissal, removeDismissal } from './dismissal-utils';
import ScoreBar from './ScoreBar';
import { TierSection, EmergingSignals } from './StackCoverageMap';
import { CoveredSection, OtherBuildTargetsSection, ProbablyFineSection } from './CollapsedSections';
import type { DepAssessment } from '../../../src-tauri/bindings/bindings/DepAssessment';
import type { BlindSpotAssessment } from '../../../src-tauri/bindings/bindings/BlindSpotAssessment';

// ============================================================================
// Main View — data shaping + layout
// ============================================================================

const BlindSpotsView = memo(function BlindSpotsView() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
  const { report, loading, error, paywalled } = useAppStore(
    useShallow((s) => ({
      report: s.blindSpotReport,
      loading: s.blindSpotsLoading,
      error: s.blindSpotsError,
      paywalled: s.blindSpotsPaywalled,
    })),
  );
  const loadBlindSpots = useAppStore((s) => s.loadBlindSpots);
  // Auto-assess: the user's toggle + whether a cloud LLM key is present. Auto
  // runs are gated to cloud-key users so we never auto-spend (or surface a
  // "no model" hint) unprompted; local-only (Ollama) and key-less users keep
  // the manual button.
  const autoAssess = useAppStore((s) => s.settings?.auto_assess_blind_spots);
  const hasLlmKey = useAppStore((s) => s.settings?.llm?.has_api_key);
  const [dismissed, setDismissed] = useState<Set<string>>(loadPersistedDismissals);
  const [lastDismissed, setLastDismissed] = useState<string | null>(null);
  const undoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => { void loadBlindSpots(); }, [loadBlindSpots]);

  // Fetch source health diagnostics — shows WHY blind spots exist
  // (adapter failing, circuit open, stale, etc.)
  const [sourceHealth, setSourceHealth] = useState<{
    total_active: number; total_failing: number; total_disabled: number;
  } | null>(null);
  useEffect(() => {
    void cmd('get_source_health').then(setSourceHealth).catch(() => {});
  }, []);

  const handleRetry = useCallback(() => { void loadBlindSpots(); }, [loadBlindSpots]);

  const handleDismiss = useCallback((id: string) => {
    setDismissed(prev => new Set(prev).add(id));
    persistDismissal(id);
    setLastDismissed(id);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
    undoTimerRef.current = setTimeout(() => setLastDismissed(null), 8000);
    recordTrustEvent({ eventType: 'dismissed', signalId: id, sourceType: 'missed_signal', notes: 'blind_spot_not_relevant' });
    void cmd('dismiss_blind_spot', { itemId: id, reason: 'not_relevant' }).catch(() => {});
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

  const handleAddWatch = useCallback((packageName: string, ecosystem: string) => {
    void cmd('add_package_watch', { packageName, ecosystem }).catch(() => {});
    void loadBlindSpots();
  }, [loadBlindSpots]);

  // Phase B: on-demand AI triage of the surfaced blind spots. One batched,
  // cached call; verdicts keyed by dep display name. `no_llm_configured` →
  // a graceful "add a key" hint rather than an error.
  const [ai, setAi] = useState<{ map: Map<string, DepAssessment>; model: string } | null>(null);
  const [aiLoading, setAiLoading] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);

  const applyAssessment = useCallback((a: BlindSpotAssessment | null) => {
    if (!a || a.assessments.length === 0) return;
    const map = new Map<string, DepAssessment>();
    for (const item of a.assessments) map.set(item.dep_name, item);
    setAi({ map, model: a.model });
  }, []);

  // `manual` distinguishes a user click (records an "acted-on" trust event)
  // from an automatic run on a dep-set change (no trust event — the user did
  // not act). Both hit the same backend command, which is cached by the
  // surfaced dep-set: an auto-run on an unchanged set returns instantly with
  // no model call.
  const runAssess = useCallback((manual: boolean) => {
    setAiLoading(true);
    setAiError(null);
    cmd('assess_blind_spots_with_ai')
      .then((res) => {
        applyAssessment(res as BlindSpotAssessment);
        if (manual) {
          recordTrustEvent({ eventType: 'acted_on', sourceType: 'gap', notes: 'blind_spot_ai_assess' });
        }
      })
      .catch((e: unknown) => {
        setAiError(String(e).includes('no_llm_configured') ? 'no_llm' : String(e));
      })
      .finally(() => setAiLoading(false));
  }, [applyAssessment]);

  const handleAssess = useCallback(() => runAssess(true), [runAssess]);

  // Persist the triage across view re-mounts / webview reloads: the backend
  // caches the last assessment in-process, so on mount we re-hydrate it. This
  // is what makes the result survive the dev-mode HMR reload loop (which drops
  // the in-flight assess callback) — and keeps it shown when navigating away
  // and back in production. Cheap: no LLM call, just a cache read.
  useEffect(() => {
    void cmd('get_cached_blind_spot_assessment')
      .then((res) => applyAssessment(res as BlindSpotAssessment | null))
      .catch(() => {});
  }, [applyAssessment]);

  const { depRows, unmatchedSignals, recommendations } = useBlindSpotsData(report, dismissed);

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

  // Auto-assess on dep-set change: when the toggle is on and a cloud LLM key is
  // present, run the triage once per distinct surfaced dep-set. The backend
  // caches by that exact set, so an unchanged set is a free instant cache read;
  // a model call happens only when a dependency newly surfaces or drops off.
  // We track the last auto-assessed signature in a ref so re-renders and the
  // hourly background scan (which doesn't change the set) don't re-fire it.
  const autoAssessedSigRef = useRef<string | null>(null);
  useEffect(() => {
    if (autoAssess === false || !hasLlmKey || paywalled) return;
    const gapNames = depRows
      .filter((d) => d.gap?.lens_hints.other_build_target !== true)
      .filter((d) => d.status === 'blind_spot' || d.status === 'falling_behind')
      .map((d) => d.name)
      .sort();
    if (gapNames.length === 0) return;
    const sig = gapNames.join('|');
    if (autoAssessedSigRef.current === sig) return;
    autoAssessedSigRef.current = sig;
    runAssess(false);
  }, [autoAssess, hasLlmKey, paywalled, depRows, runAssess]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-text-secondary text-sm">
        {t('blindspots.loading')}
      </div>
    );
  }
  // Tier gate is a paywall, not a fault — show an upgrade path (now with the
  // honest free teaser counts), never the red error banner below. (Mirrors
  // PreemptionView; the shared isSignalGateError classifier routes the gate
  // here instead of into blindSpotsError.)
  if (paywalled) {
    return <BlindSpotsPaywall />;
  }
  if (error) {
    const isTimeoutError = /timed?\s*out|deadline/i.test(error);
    return (
      <div className="space-y-4" role="tabpanel" id="view-panel-blindspots" aria-labelledby="tab-blindspots">
        <header className="mb-2">
          <h1 className="text-xl font-semibold text-text-primary tracking-tight">{t('blindspots.title')}</h1>
          <p className="text-sm text-text-muted mt-1">{t('blindspots.subtitle')}</p>
        </header>
        <div className="bg-bg-secondary rounded-lg border border-red-500/20 px-5 py-5">
          <div className="flex items-start gap-4">
            <div className="w-9 h-9 rounded-full bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
              <span className="text-red-300 text-sm">!</span>
            </div>
            <div className="min-w-0 flex-1 space-y-3">
              <div className="space-y-1">
                <h3 className="text-sm font-medium text-text-primary">{t('blindspots.error.title')}</h3>
                <p className="text-sm text-text-muted">
                  {isTimeoutError ? t('blindspots.error.timeout') : t('blindspots.error.subtitle')}
                </p>
              </div>
              <p className="text-xs text-red-300">{error}</p>
              <button
                onClick={handleRetry}
                className="px-3 py-1.5 text-sm text-text-primary bg-bg-tertiary border border-border rounded-lg hover:border-red-500/30 hover:text-red-300 transition-colors"
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
  // Phase 2c: deps whose coverage gap applies only to a build target the user
  // does not build on the host are pulled into a collapsed "other build targets"
  // group — surfaced, de-prioritised, never hidden — so they don't crowd the
  // host-relevant stack/ecosystem tiers.
  const isOtherTarget = (d: typeof depRows[number]) => d.gap?.lens_hints.other_build_target === true;
  const otherTargetDeps = depRows.filter(isOtherTarget);
  const normalDeps = depRows.filter(d => !isOtherTarget(d));
  const stackDeps = normalDeps.filter(d => d.status === 'blind_spot');
  const ecosystemDeps = normalDeps.filter(d => d.status === 'falling_behind');
  const coveredDeps = normalDeps.filter(d => d.status === 'well_covered');

  const hasProblems = stackDeps.length > 0 || ecosystemDeps.length > 0;
  const hasContent = hasProblems || unmatchedSignals.length > 0;
  const dataFreshness = report.data_freshness;

  // Phase B: when the AI triage has run, split the host-relevant gap deps into
  // "worth reviewing" vs "probably fine" by the model's verdict. A dep that
  // wasn't assessed defaults to worth-reviewing — we never hide what wasn't
  // judged. Recommendations are keyed by display name for the rows to show.
  const aiMap = ai?.map ?? null;
  const aiActive = aiMap !== null;
  const gapDeps = [...stackDeps, ...ecosystemDeps];
  const worthReviewing = aiActive ? gapDeps.filter(d => aiMap.get(d.name)?.worth_reviewing !== false) : [];
  const probablyFine = aiActive ? gapDeps.filter(d => aiMap.get(d.name)?.worth_reviewing === false) : [];
  const aiRecs = aiActive
    ? new Map(Array.from(aiMap.entries()).map(([k, v]) => [k, v.recommendation] as const))
    : undefined;

  return (
    <div className="space-y-4 pb-8" role="tabpanel" id="view-panel-blindspots" aria-labelledby="tab-blindspots">
      <header className="mb-2">
        <h1 className="text-xl font-semibold text-text-primary tracking-tight">{t('blindspots.title')}</h1>
        <p className="text-sm text-text-muted mt-1">{t('blindspots.subtitle')}</p>
      </header>
      <ScoreBar score={score} />
      {totalTracked > 0 && (
        <div className="flex items-center gap-4 px-4 py-2.5 rounded-lg bg-bg-secondary border border-border -mt-1">
          {hasContent && (
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
          )}
          <span className="text-xs text-text-muted tabular-nums ms-auto">
            {t('blindspots.stats.tracked', { count: totalTracked })}
            {(report.weak_match_count ?? 0) > 0 && (
              <span className="text-text-muted/60 ml-2">
                {t('blindspots.stats.weakHidden', { count: report.weak_match_count ?? undefined })}
              </span>
            )}
          </span>
        </div>
      )}
      {hasProblems && (
        <div className="flex items-center gap-2 flex-wrap -mt-1">
          <button
            onClick={handleAssess}
            disabled={aiLoading}
            className="inline-flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-md border border-amber-500/30 bg-amber-500/10 text-amber-300 hover:bg-amber-500/20 disabled:opacity-60 transition-colors"
          >
            {aiLoading ? t('blindspots.ai.assessing') : t('blindspots.ai.assess')}
          </button>
          {ai && !aiLoading && (
            <span className="text-[10px] text-text-muted">{t('blindspots.ai.assessedWith', { model: ai.model })}</span>
          )}
          {aiError === 'no_llm' && (
            <span className="text-[10px] text-text-muted">{t('blindspots.ai.noModel')}</span>
          )}
          {aiError && aiError !== 'no_llm' && (
            <span className="text-[10px] text-red-400">{t('blindspots.ai.failed')}</span>
          )}
        </div>
      )}
      {sourceHealth && sourceHealth.total_failing > 0 && (
        <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-orange-500/10 border border-orange-500/20 text-xs text-orange-400">
          <span className="w-1.5 h-1.5 rounded-full bg-orange-400 shrink-0" />
          {t('blindspots.sourceHealth.failing', { count: sourceHealth.total_failing })}
          {sourceHealth.total_disabled > 0 && <span className="text-text-muted ml-1">{t('blindspots.sourceHealth.stale', { count: sourceHealth.total_disabled })}</span>}
        </div>
      )}
      {dataFreshness?.is_stale && (
        <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-amber-500/10 border border-amber-500/20 text-xs text-amber-300">
          <span className="w-1.5 h-1.5 rounded-full bg-amber-300 shrink-0" />
          {dataFreshness.newest_item_age_hours != null
            ? t('blindspots.staleDataAge', { days: Math.floor(dataFreshness.newest_item_age_hours / 24) })
            : t('blindspots.staleData')}
        </div>
      )}
      {lastDismissed !== null && (
        <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20 animate-in fade-in">
          <span className="text-xs text-amber-400">{t('blindspots.dismissed')}</span>
          <button
            type="button"
            onClick={handleUndo}
            className="text-xs font-medium text-amber-400 hover:text-text-primary underline-offset-2 hover:underline transition-colors"
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
        ) : score > 0 && score <= 10 ? (
          /* Genuinely excellent: the system actively evaluated and found very few issues */
          <div className="bg-bg-secondary rounded-lg border border-emerald-500/20 px-5 py-6">
            <div className="flex items-start gap-4">
              <div className="w-9 h-9 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center shrink-0">
                {/* eslint-disable-next-line i18next/no-literal-string */}
                <span className="text-emerald-400 text-sm">&#10003;</span>
              </div>
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-medium text-text-primary">{t('blindspots.scoreContext.excellent')}</h3>
                {recommendations.length > 0 && (
                  <p className="text-xs text-text-muted mt-1">{recommendations[0]!.explanation}</p>
                )}
              </div>
            </div>
          </div>
        ) : (
          /* score === 0 with no items: backend returned empty, don't claim "excellent" */
          <div className="bg-bg-secondary rounded-lg border border-border px-5 py-6">
            <div className="flex items-start gap-4">
              <div className="w-9 h-9 rounded-full bg-bg-tertiary border border-border flex items-center justify-center shrink-0">
                <span className="text-text-muted text-sm">&mdash;</span>
              </div>
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-medium text-text-secondary">
                  {t('blindspots.empty')}
                </h3>
                {recommendations.length > 0 && (
                  <p className="text-xs text-text-muted mt-1">{recommendations[0]!.explanation}</p>
                )}
              </div>
            </div>
          </div>
        )
      ) : (
        <>
          {aiActive ? (
            /* Phase B: AI triage active — one "worth reviewing" group + a
               collapsed "probably fine" bucket, replacing the stack/ecosystem
               split. Each row carries its one-line AI recommendation. */
            <>
              {worthReviewing.length > 0 && (
                <TierSection
                  dotColor="#EF4444"
                  borderColor="rgba(239, 68, 68, 0.2)"
                  title={t('blindspots.ai.worthReviewing')}
                  subtitle={t('blindspots.ai.worthReviewingSubtitle', { count: worthReviewing.length })}
                  badgeText={t('blindspots.tier.needsAttention')}
                  badgeColor="#EF4444"
                  depRows={worthReviewing}
                  onDismissSignal={handleDismiss}
                  onAddWatch={handleAddWatch}
                  emptyText={t('blindspots.tier.stackEmpty')}
                  aiRecommendations={aiRecs}
                />
              )}
              <ProbablyFineSection
                depRows={probablyFine}
                onDismissSignal={handleDismiss}
                onAddWatch={handleAddWatch}
                aiRecommendations={aiRecs}
              />
            </>
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
                  onAddWatch={handleAddWatch}
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
                  onAddWatch={handleAddWatch}
                  emptyText={t('blindspots.tier.ecosystemEmpty')}
                />
              )}
            </>
          )}
          <EmergingSignals items={unmatchedSignals} onDismiss={handleDismiss} />
          {recommendations.length > 0 && (
            <div className="space-y-1.5">
              {recommendations.map(rec => (
                <div key={rec.id} className="px-4 py-2.5 bg-bg-secondary rounded-lg border border-border group">
                  <div className="flex items-start gap-2">
                    <div className="flex-1 min-w-0">
                      <p className="text-xs text-text-secondary">{rec.title}</p>
                      <p className="text-[11px] text-text-muted mt-0.5">{rec.explanation}</p>
                    </div>
                    {/* eslint-disable i18next/no-literal-string */}
                    <button
                      onClick={() => handleDismiss(rec.id)}
                      className="text-xs text-text-muted hover:text-red-400 opacity-0 group-hover:opacity-100 transition-all shrink-0 px-1.5 py-1 rounded hover:bg-red-500/10"
                      title={t('blindspots.signal.notRelevant')}
                    >
                      ✕
                    </button>
                    {/* eslint-enable i18next/no-literal-string */}
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
      <CoveredSection depRows={coveredDeps} onDismissSignal={handleDismiss} />
      <OtherBuildTargetsSection
        depRows={otherTargetDeps}
        onDismissSignal={handleDismiss}
        onAddWatch={handleAddWatch}
      />
    </div>
  );
});

export default BlindSpotsView;

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useRef, useCallback, useMemo, useState, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { useColdStartGate } from '../../hooks/use-cold-start-gate';
import { URGENCY_ORDER, ItemCard } from './PreemptionCard';
import { PreemptionTierSection } from './PreemptionTierSection';
import { PreemptionFreeFloorNotice } from './PreemptionFreeFloorNotice';
import { SignalUpgradeCTA } from '../SignalUpgradeCTA';

// The Upgrade Plan is a ranked list that can run to 100+ steps on a large
// stack. The list transport ships only this many (keep in sync with
// LIST_PLAN_STEP_CAP in src-tauri/src/evidence/list_transport.rs); the rest
// are one click away — expanding refetches with `fullPlan`. Nothing is
// suppressed (every step is in the persisted snapshot the `4da plan` CLI /
// MCP handoff read).
const UPGRADE_PLAN_VISIBLE_CAP = 25;

const PreemptionView = memo(function PreemptionView() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
  const surfacedRef = useRef(new Set<string>());
  const undoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [showOtherTargets, setShowOtherTargets] = useState(false);

  const { feed, loading, error, paywalled, lastDismissed } = useAppStore(
    useShallow(s => ({
      feed: s.preemptionFeed,
      loading: s.preemptionLoading,
      error: s.preemptionError,
      paywalled: s.preemptionPaywalled,
      lastDismissed: s.preemptionLastDismissed,
    })),
  );
  const loadPreemption = useAppStore(s => s.loadPreemption);
  const dismissPreemptionItem = useAppStore(s => s.dismissPreemptionItem);
  const undoPreemptionDismissal = useAppStore(s => s.undoPreemptionDismissal);
  const clearPreemptionUndo = useAppStore(s => s.clearPreemptionUndo);
  const expandPreemptionPlan = useAppStore(s => s.expandPreemptionPlan);

  useEffect(() => {
    void loadPreemption();
  }, [loadPreemption]);

  const handleDismiss = useCallback((id: string) => {
    // Persist + refetch: the backend re-applies THE visibility filter, so the
    // card and the header counts move in the same response (AD-035).
    void dismissPreemptionItem(id);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
    undoTimerRef.current = setTimeout(() => clearPreemptionUndo(), 8000);
  }, [dismissPreemptionItem, clearPreemptionUndo]);

  const handleUndo = useCallback(() => {
    void undoPreemptionDismissal();
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
  }, [undoPreemptionDismissal]);

  // Grouping ONLY — the backend already applied the one visibility filter
  // (dismissals, plan-covered regrouping) and computed the matching counts
  // (AD-035). Every item received here is rendered in exactly one section;
  // filtering or re-counting client-side would recreate the count drift the
  // 2026-08-31 audit caught (header 12/41/120 vs payload 15/67/149).
  const { planItems, verifiedItems, assessedItems, developingItems, otherTargetItems } = useMemo(() => {
    const sorted = (feed?.items ?? [])
      .slice()
      .sort(
        (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
      );
    const plan: EvidenceItem[] = [];
    const verified: EvidenceItem[] = [];
    const assessed: EvidenceItem[] = [];
    const developing: EvidenceItem[] = [];
    const otherTarget: EvidenceItem[] = [];
    for (const item of sorted) {
      if (item.lens_hints.upgrade_plan) {
        plan.push(item);
      } else if (item.lens_hints.other_build_target) {
        otherTarget.push(item);
      } else if (item.confidence.provenance === 'osv_verified') {
        verified.push(item);
      } else if (item.confidence.provenance === 'llm_assessed') {
        assessed.push(item);
      } else {
        developing.push(item);
      }
    }
    return {
      planItems: plan,
      verifiedItems: verified,
      assessedItems: assessed,
      developingItems: developing,
      otherTargetItems: otherTarget,
    };
  }, [feed]);

  // Counts come from the feed verbatim — same filter, same numbers as the
  // cards below (the backend excludes other-build-target rows from the
  // critical/high tallies, exactly as this bar always displayed them).
  const criticalCount = feed?.critical_count ?? 0;
  const highCount = feed?.high_count ?? 0;
  const totalAlerts = feed?.total ?? 0;
  // The list transport ships the top-ranked plan steps and holds the
  // collapsed tail back; `total` still counts it. The difference IS the
  // held-back step count (pinned by plan_cap_holds_back_the_collapsed_tail
  // in list_transport_tests.rs).
  const planHeldBack = feed ? Math.max(0, feed.total - feed.items.length) : 0;

  // Free security floor: the backend served Tier 1 (OSV-verified) only.
  // Render the floor normally plus a compact locked-tiers notice — never a
  // full-page paywall over real security data.
  const isFreeFloor = feed?.tier_scope === 'free_floor';

  return (
    <div className="space-y-5" role="tabpanel" id="view-panel-preemption" aria-labelledby="tab-preemption">
      <header>
        <h1 className="text-xl font-semibold text-text-primary tracking-tight">{t('preemption.title')}</h1>
        <p className="text-sm text-text-muted mt-1">{t('preemption.subtitle')}</p>
      </header>

      {/* Stale-backend fallback only: since the 2026-06-12 tier rebalance the
          backend never gates get_preemption_alerts (free tier gets the OSV
          floor), so this branch fires only against an older backend binary.
          A tier gate is a paywall, not a fault — upgrade path, never a red
          error banner. (loading/error/feed are all falsy in this state.) */}
      {paywalled && (
        <div className="flex flex-col items-center justify-center py-20 text-center gap-3">
          <div className="w-12 h-12 rounded-full bg-accent-gold/10 border border-accent-gold/20 flex items-center justify-center mb-1">
            <span className="text-accent-gold text-lg" aria-hidden="true">&#x1F512;</span>
          </div>
          <p className="text-sm font-medium text-text-primary">{t('preemption.locked.title')}</p>
          <p className="text-xs text-text-muted max-w-sm">{t('preemption.locked.subtitle')}</p>
          <div className="mt-1">
            <SignalUpgradeCTA source="preemption-paywall" />
          </div>
        </div>
      )}

      {loading && !feed && (
        <div className="flex items-center justify-center py-16">
          <p className="text-sm text-text-muted animate-pulse">{t('preemption.loading')}</p>
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-400">
          {error}
        </div>
      )}

      {feed && totalAlerts === 0 && !isColdStart && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <div className="w-12 h-12 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center mb-3">
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-emerald-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm font-medium text-text-primary mb-1">{t('preemption.empty.title')}</p>
          <p className="text-xs text-text-muted">{t('preemption.empty.subtitle')}</p>
        </div>
      )}

      {feed && totalAlerts > 0 && (
        <>
          <div className="flex items-center gap-4 px-4 py-3 rounded-lg bg-bg-secondary border border-border">
            <div className="flex items-center gap-3 text-xs">
              {verifiedItems.length > 0 && (
                <span className="inline-flex items-center gap-1.5 text-emerald-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                  {verifiedItems.length} {t('preemption.badge.verified').toLowerCase()}
                </span>
              )}
              {criticalCount > 0 && (
                <span className="inline-flex items-center gap-1.5 text-red-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
                  {criticalCount} {t('preemption.urgency.critical').toLowerCase()}
                </span>
              )}
              {highCount > 0 && (
                <span className="inline-flex items-center gap-1.5 text-orange-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-orange-400" />
                  {highCount} {t('preemption.urgency.high').toLowerCase()}
                </span>
              )}
            </div>
            <span className="ms-auto text-xs text-text-muted tabular-nums">
              {t('preemption.alert', { count: totalAlerts })}
            </span>
          </div>

          {lastDismissed !== null && (
            <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20 animate-in fade-in">
              <span className="text-xs text-amber-400">{t('preemption.dismissed')}</span>
              <button
                type="button"
                onClick={handleUndo}
                className="text-xs font-medium text-amber-400 hover:text-text-primary underline-offset-2 hover:underline transition-colors"
              >
                {t('preemption.action.undo')}
              </button>
            </div>
          )}

          {/* Phase 1 dependency intelligence: the ranked Upgrade Plan — which
              upgrade matters most, highest impact first. Signal-tier (the free
              floor never contains plan steps); absent entirely when empty.
              The collapsed tail beyond the cap lives server-side; expanding
              refetches the full plan (AD-035). */}
          {planItems.length > 0 && (
            <PreemptionTierSection
              dotColor="#D4AF37"
              borderColor="rgba(212, 175, 55, 0.25)"
              title={t('preemption.upgradePlan.title')}
              subtitle={t('preemption.upgradePlan.subtitle', { count: planItems.length + planHeldBack })}
              items={planItems}
              surfacedRef={surfacedRef}
              onDismiss={handleDismiss}
              emptyText={t('preemption.upgradePlan.empty')}
              maxVisible={UPGRADE_PLAN_VISIBLE_CAP}
              hiddenExtra={planHeldBack}
              onExpand={expandPreemptionPlan}
              showMoreLabel={hidden => t('preemption.evidence.showMore', { count: hidden })}
            />
          )}

          {verifiedItems.length > 0 && (
            <PreemptionTierSection
              dotColor="#22C55E"
              borderColor="rgba(34, 197, 94, 0.2)"
              title={t('preemption.tier.verified')}
              subtitle={t('preemption.tier.verifiedSubtitle', { count: verifiedItems.length })}
              items={verifiedItems}
              surfacedRef={surfacedRef}
              onDismiss={handleDismiss}
              emptyText={t('preemption.tier.verifiedEmpty')}
            />
          )}

          {assessedItems.length > 0 && (
            <PreemptionTierSection
              dotColor="#3B82F6"
              borderColor="rgba(59, 130, 246, 0.2)"
              title={t('preemption.tier.assessed')}
              subtitle={t('preemption.tier.assessedSubtitle', { count: assessedItems.length })}
              items={assessedItems}
              surfacedRef={surfacedRef}
              onDismiss={handleDismiss}
              emptyText={t('preemption.tier.assessedEmpty')}
            />
          )}

          {developingItems.length > 0 && (
            <PreemptionTierSection
              dotColor="#8A8A8A"
              borderColor="rgba(138, 138, 138, 0.15)"
              title={t('preemption.tier.developing')}
              subtitle={t('preemption.tier.developingSubtitle', { count: developingItems.length })}
              items={developingItems}
              surfacedRef={surfacedRef}
              onDismiss={handleDismiss}
              emptyText={t('preemption.tier.developingEmpty')}
            />
          )}

          {/* Phase 2c: advisories for deps the user does not build on this host.
              Collapsed by default — surfaced (a cross-platform dev can open it),
              never urgent, never hidden. */}
          {otherTargetItems.length > 0 && (
            <section className="rounded-lg border border-border bg-bg-secondary overflow-hidden" aria-label={t('preemption.otherTargets.title')}>
              <button
                type="button"
                onClick={() => setShowOtherTargets(v => !v)}
                aria-expanded={showOtherTargets}
                className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
              >
                <div className="w-2 h-2 rounded-full shrink-0 bg-[#8A8A8A]" />
                <h3 className="text-sm font-medium text-text-secondary flex-1 text-left">
                  {t('preemption.otherTargets.show', { count: otherTargetItems.length })}
                </h3>
                <span className="text-[10px] text-text-muted">
                  {showOtherTargets ? t('preemption.otherTargets.hide') : t('preemption.otherTargets.expand')}
                </span>
              </button>
              {showOtherTargets && (
                <div className="border-t border-border p-4 space-y-4">
                  {otherTargetItems.map(item => (
                    <ItemCard key={item.id} item={item} surfacedRef={surfacedRef} onDismiss={handleDismiss} />
                  ))}
                </div>
              )}
            </section>
          )}
        </>
      )}

      {feed && isFreeFloor && !isColdStart && <PreemptionFreeFloorNotice />}
    </div>
  );
});

export default PreemptionView;

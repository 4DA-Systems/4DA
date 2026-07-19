// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useRef, useState, useCallback, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { useColdStartGate } from '../../hooks/use-cold-start-gate';
import { URGENCY_ORDER, ItemCard } from './PreemptionCard';
import { PreemptionTierSection } from './PreemptionTierSection';
import { PreemptionFreeFloorNotice } from './PreemptionFreeFloorNotice';
import { SignalUpgradeCTA } from '../SignalUpgradeCTA';

const DISMISS_STORAGE_KEY = 'preemption_dismissed';
const DISMISS_TTL_MS = 7 * 24 * 60 * 60 * 1000;

// The Upgrade Plan is a ranked list that can run to 100+ steps on a large stack.
// Show the top-ranked steps that matter most; collapse the rest behind a "show
// more" control so the human surface stays scannable (doctrine: which upgrades
// MATTER, not an exhaustive wall). The full plan is never suppressed — every
// step is in the persisted snapshot the `4da plan` CLI / MCP handoff read.
const UPGRADE_PLAN_VISIBLE_CAP = 25;

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

const PreemptionView = memo(function PreemptionView() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
  const surfacedRef = useRef(new Set<string>());
  const [dismissedIds, setDismissedIds] = useState<Set<string>>(loadPersistedDismissals);
  const [lastDismissed, setLastDismissed] = useState<string | null>(null);
  const [showOtherTargets, setShowOtherTargets] = useState(false);
  const undoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { feed, loading, error, paywalled } = useAppStore(
    useShallow(s => ({
      feed: s.preemptionFeed,
      loading: s.preemptionLoading,
      error: s.preemptionError,
      paywalled: s.preemptionPaywalled,
    })),
  );
  const loadPreemption = useAppStore(s => s.loadPreemption);

  useEffect(() => {
    void loadPreemption();
  }, [loadPreemption]);

  const handleDismiss = useCallback((id: string) => {
    setDismissedIds(prev => new Set(prev).add(id));
    persistDismissal(id);
    setLastDismissed(id);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
    undoTimerRef.current = setTimeout(() => setLastDismissed(null), 8000);
  }, []);

  const handleUndo = useCallback(() => {
    if (!lastDismissed) return;
    setDismissedIds(prev => {
      const next = new Set(prev);
      next.delete(lastDismissed);
      return next;
    });
    removeDismissal(lastDismissed);
    setLastDismissed(null);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
  }, [lastDismissed]);

  const { planItems, verifiedItems, assessedItems, developingItems, otherTargetItems, criticalCount, highCount } = useMemo(() => {
    const visible = (feed?.items ?? [])
      .filter(item => !dismissedIds.has(item.id))
      .slice()
      .sort(
        (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
      );

    // Phase 1 dependency intelligence: ranked "Upgrade Plan" steps (Signal tier;
    // the free floor never contains them). Rendered as their own section above
    // the tiers. The stable urgency sort preserves the backend's within-urgency
    // ranking (fixable-now first, widest blast radius first).
    const plan: EvidenceItem[] = visible.filter(item => item.lens_hints.upgrade_plan);
    // A package represented by a plan step must not ALSO appear as its
    // per-package advisory alert in the verified tier below — same facts, two
    // cards (the plan step carries the same advisory citations plus the action
    // framing). Regrouped, not suppressed: free tier has no plan items and is
    // untouched, and non-verified/other-target items never match this rule.
    const planPackages = new Set(
      plan.flatMap(item => item.affected_deps.map(dep => dep.toLowerCase())),
    );

    const verified: EvidenceItem[] = [];
    const assessed: EvidenceItem[] = [];
    const developing: EvidenceItem[] = [];
    // Phase 2c: advisories relevant only to a build target the user does not
    // build on the host are pulled out of the main tiers into a collapsed
    // "other build targets" group — surfaced, de-prioritised, never hidden.
    const otherTarget: EvidenceItem[] = [];
    // Count urgencies from the VISIBLE (post-dismissal) set, not feed.*_count from the
    // backend — otherwise dismissing the only critical leaves the bar reading "1 critical"
    // over an empty list (the count must match the cards beneath it). Regrouped
    // duplicates are skipped BEFORE counting for the same reason.
    let critical = 0;
    let high = 0;
    for (const item of visible) {
      if (item.lens_hints.upgrade_plan) {
        if (item.urgency === 'critical') critical += 1;
        else if (item.urgency === 'high') high += 1;
        continue; // already in `plan`
      }
      if (item.lens_hints.other_build_target) {
        otherTarget.push(item);
        continue;
      }
      const coveredByPlan =
        planPackages.size > 0 &&
        item.confidence.provenance === 'osv_verified' &&
        item.affected_deps.length > 0 &&
        item.affected_deps.every(dep => planPackages.has(dep.toLowerCase()));
      if (coveredByPlan) continue;
      if (item.urgency === 'critical') critical += 1;
      else if (item.urgency === 'high') high += 1;
      if (item.confidence.provenance === 'osv_verified') {
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
      criticalCount: critical,
      highCount: high,
    };
  }, [feed, dismissedIds]);

  const totalVisible =
    planItems.length +
    verifiedItems.length + assessedItems.length + developingItems.length + otherTargetItems.length;
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
            <SignalUpgradeCTA />
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

      {feed && totalVisible === 0 && !isColdStart && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <div className="w-12 h-12 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center mb-3">
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-emerald-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm font-medium text-text-primary mb-1">{t('preemption.empty.title')}</p>
          <p className="text-xs text-text-muted">{t('preemption.empty.subtitle')}</p>
        </div>
      )}

      {feed && totalVisible > 0 && (
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
              {t('preemption.alert', { count: totalVisible })}
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
              floor never contains plan steps); absent entirely when empty. */}
          {planItems.length > 0 && (
            <PreemptionTierSection
              dotColor="#D4AF37"
              borderColor="rgba(212, 175, 55, 0.25)"
              title={t('preemption.upgradePlan.title')}
              subtitle={t('preemption.upgradePlan.subtitle', { count: planItems.length })}
              items={planItems}
              surfacedRef={surfacedRef}
              onDismiss={handleDismiss}
              emptyText={t('preemption.upgradePlan.empty')}
              maxVisible={UPGRADE_PLAN_VISIBLE_CAP}
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

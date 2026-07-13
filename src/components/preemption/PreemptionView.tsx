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

// "Not affected" is a REVIEWABLE bucket, not a black hole. Marking an advisory
// "Not affected" parks it here (persistent, no auto-expiry) so the user can
// always find what they dismissed and, if they were wrong, restore it — or, if
// they are sure, delete it permanently. A permanently-deleted id is suppressed
// even when the backend re-serves the same advisory on the next scan.
const NOT_AFFECTED_KEY = 'preemption_not_affected';
const DELETED_KEY = 'preemption_deleted';

interface NotAffectedEntry {
  id: string;
  ts: number;
  title: string;
  deps: string[];
}

function loadNotAffected(): NotAffectedEntry[] {
  try {
    const raw = localStorage.getItem(NOT_AFFECTED_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as NotAffectedEntry[];
    return Array.isArray(parsed) ? parsed : [];
  } catch { return []; }
}

function saveNotAffected(entries: NotAffectedEntry[]) {
  try { localStorage.setItem(NOT_AFFECTED_KEY, JSON.stringify(entries)); }
  catch { /* non-fatal */ }
}

function loadDeleted(): Set<string> {
  try {
    const raw = localStorage.getItem(DELETED_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw) as string[];
    return new Set(Array.isArray(parsed) ? parsed : []);
  } catch { return new Set(); }
}

function saveDeleted(ids: Set<string>) {
  try { localStorage.setItem(DELETED_KEY, JSON.stringify([...ids])); }
  catch { /* non-fatal */ }
}

const PreemptionView = memo(function PreemptionView() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
  const surfacedRef = useRef(new Set<string>());
  const [notAffected, setNotAffected] = useState<NotAffectedEntry[]>(loadNotAffected);
  const [deletedIds, setDeletedIds] = useState<Set<string>>(loadDeleted);
  const [lastDismissed, setLastDismissed] = useState<string | null>(null);
  const [showOtherTargets, setShowOtherTargets] = useState(false);
  const [showNotAffected, setShowNotAffected] = useState(false);
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

  // Mark "Not affected": park the advisory in the reviewable bucket. Snapshot
  // title + deps so the bucket still renders if the advisory later drops out of
  // the feed (e.g. the dependency was upgraded out of the affected range).
  const handleDismiss = useCallback((id: string) => {
    const item = feed?.items.find(i => i.id === id);
    setNotAffected(prev => {
      if (prev.some(e => e.id === id)) return prev;
      const next: NotAffectedEntry[] = [
        { id, ts: Date.now(), title: item?.title ?? id, deps: item?.affected_deps ?? [] },
        ...prev,
      ];
      saveNotAffected(next);
      return next;
    });
    setLastDismissed(id);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
    undoTimerRef.current = setTimeout(() => setLastDismissed(null), 8000);
  }, [feed]);

  const handleUndo = useCallback(() => {
    if (!lastDismissed) return;
    setNotAffected(prev => {
      const next = prev.filter(e => e.id !== lastDismissed);
      saveNotAffected(next);
      return next;
    });
    setLastDismissed(null);
    if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
  }, [lastDismissed]);

  // Bucket actions: bring an advisory back to the live feed, or suppress it for
  // good (survives re-scans via the deleted-id set).
  const handleRestore = useCallback((id: string) => {
    setNotAffected(prev => {
      const next = prev.filter(e => e.id !== id);
      saveNotAffected(next);
      return next;
    });
  }, []);

  const handleDeleteForever = useCallback((id: string) => {
    setNotAffected(prev => {
      const next = prev.filter(e => e.id !== id);
      saveNotAffected(next);
      return next;
    });
    setDeletedIds(prev => {
      const next = new Set(prev).add(id);
      saveDeleted(next);
      return next;
    });
  }, []);

  // Items in the "Not affected" bucket or permanently deleted are pulled out of
  // the live tiers (but the bucket ones remain reachable in the review section).
  const hiddenIds = useMemo(() => {
    const s = new Set(deletedIds);
    for (const e of notAffected) s.add(e.id);
    return s;
  }, [notAffected, deletedIds]);

  const { verifiedItems, assessedItems, developingItems, otherTargetItems, criticalCount, highCount } = useMemo(() => {
    const visible = (feed?.items ?? [])
      .filter(item => !hiddenIds.has(item.id))
      .slice()
      .sort(
        (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
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
    // over an empty list (the count must match the cards beneath it).
    let critical = 0;
    let high = 0;
    for (const item of visible) {
      if (item.lens_hints.other_build_target) {
        otherTarget.push(item);
        continue;
      }
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
      verifiedItems: verified,
      assessedItems: assessed,
      developingItems: developing,
      otherTargetItems: otherTarget,
      criticalCount: critical,
      highCount: high,
    };
  }, [feed, hiddenIds]);

  const totalVisible =
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
              <span className="text-xs text-amber-400">{t('preemption.notAffected.moved', 'Moved to "Not affected"')}</span>
              <button
                type="button"
                onClick={handleUndo}
                className="text-xs font-medium text-amber-400 hover:text-text-primary underline-offset-2 hover:underline transition-colors"
              >
                {t('preemption.action.undo')}
              </button>
            </div>
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

      {/* "Not affected" review bucket — everything the user marked not affected,
          kept reachable (never a black hole). Restore if they were wrong, or
          delete permanently if they are sure. Rendered outside the active-feed
          block so it stays available even when the live feed is empty. */}
      {notAffected.length > 0 && (
        <section
          className="rounded-lg border border-border bg-bg-secondary overflow-hidden"
          aria-label={t('preemption.notAffected.title', 'Not affected')}
        >
          <button
            type="button"
            onClick={() => setShowNotAffected(v => !v)}
            aria-expanded={showNotAffected}
            className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
          >
            <div className="w-2 h-2 rounded-full shrink-0 bg-[#8A8A8A]" />
            <h3 className="text-sm font-medium text-text-secondary flex-1 text-left">
              {t('preemption.notAffected.title', 'Not affected')} ({notAffected.length})
            </h3>
            <span className="text-[10px] text-text-muted">
              {showNotAffected
                ? t('preemption.notAffected.hide', 'hide')
                : t('preemption.notAffected.show', 'review')}
            </span>
          </button>
          {showNotAffected && (
            <div className="border-t border-border">
              <p className="px-4 py-2 text-[11px] text-text-muted">
                {t(
                  'preemption.notAffected.hint',
                  'Advisories you marked not affected. Restore one if you were wrong, or delete it permanently if you are sure.',
                )}
              </p>
              <ul className="divide-y divide-border">
                {notAffected.map(entry => (
                  <li key={entry.id} className="px-4 py-3 flex items-start gap-3">
                    <div className="flex-1 min-w-0">
                      <p className="text-xs text-text-secondary truncate" title={entry.title}>
                        {entry.title}
                      </p>
                      {entry.deps.length > 0 && (
                        <p className="mt-0.5 text-[10px] font-mono text-text-muted truncate">
                          {entry.deps.join(', ')}
                        </p>
                      )}
                    </div>
                    <div className="flex shrink-0 gap-1.5">
                      <button
                        type="button"
                        onClick={() => handleRestore(entry.id)}
                        className="px-2 py-1 text-[10px] rounded border border-border text-text-secondary hover:text-text-primary hover:border-text-primary/20 transition-colors"
                      >
                        {t('preemption.notAffected.restore', 'Restore')}
                      </button>
                      <button
                        type="button"
                        onClick={() => handleDeleteForever(entry.id)}
                        title={t('preemption.notAffected.deleteHint', 'Delete permanently — will not resurface on future scans')}
                        className="px-2 py-1 text-[10px] rounded border border-red-500/25 text-red-400/80 hover:text-red-400 hover:border-red-500/50 transition-colors"
                      >
                        {t('preemption.notAffected.delete', 'Delete')}
                      </button>
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}

      {feed && isFreeFloor && !isColdStart && <PreemptionFreeFloorNotice />}
    </div>
  );
});

export default PreemptionView;

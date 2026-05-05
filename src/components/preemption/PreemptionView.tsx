// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useRef, useState, useCallback, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { useColdStartGate } from '../../hooks/use-cold-start-gate';
import { URGENCY_ORDER } from './PreemptionCard';
import { PreemptionTierSection } from './PreemptionTierSection';

const DISMISS_STORAGE_KEY = 'preemption_dismissed';
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

const PreemptionView = memo(function PreemptionView() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
  const surfacedRef = useRef(new Set<string>());
  const [dismissedIds, setDismissedIds] = useState<Set<string>>(loadPersistedDismissals);
  const [lastDismissed, setLastDismissed] = useState<string | null>(null);
  const undoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { feed, loading, error } = useAppStore(
    useShallow(s => ({
      feed: s.preemptionFeed,
      loading: s.preemptionLoading,
      error: s.preemptionError,
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

  const { verifiedItems, assessedItems, developingItems } = useMemo(() => {
    const visible = (feed?.items ?? [])
      .filter(item => !dismissedIds.has(item.id))
      .slice()
      .sort(
        (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
      );

    const verified: EvidenceItem[] = [];
    const assessed: EvidenceItem[] = [];
    const developing: EvidenceItem[] = [];
    for (const item of visible) {
      if (item.confidence.provenance === 'osv_verified') {
        verified.push(item);
      } else if (item.confidence.provenance === 'llm_assessed') {
        assessed.push(item);
      } else {
        developing.push(item);
      }
    }
    return { verifiedItems: verified, assessedItems: assessed, developingItems: developing };
  }, [feed, dismissedIds]);

  const totalVisible = verifiedItems.length + assessedItems.length + developingItems.length;

  return (
    <div className="space-y-5" role="tabpanel" id="view-panel-preemption">
      <header>
        <h1 className="text-xl font-semibold text-white tracking-tight">{t('preemption.title')}</h1>
        <p className="text-sm text-text-muted mt-1">{t('preemption.subtitle')}</p>
      </header>

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
            <span className="text-emerald-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm font-medium text-white mb-1">{t('preemption.empty.title')}</p>
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
              {feed.critical_count > 0 && (
                <span className="inline-flex items-center gap-1.5 text-red-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
                  {feed.critical_count} {t('preemption.urgency.critical').toLowerCase()}
                </span>
              )}
              {feed.high_count > 0 && (
                <span className="inline-flex items-center gap-1.5 text-orange-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-orange-400" />
                  {feed.high_count} {t('preemption.urgency.high').toLowerCase()}
                </span>
              )}
            </div>
            <span className="ms-auto text-xs text-text-muted tabular-nums">
              {t('preemption.alert', { count: feed.total })}
            </span>
          </div>

          {lastDismissed !== null && (
            <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20 animate-in fade-in">
              <span className="text-xs text-amber-400">{t('preemption.dismissed')}</span>
              <button
                type="button"
                onClick={handleUndo}
                className="text-xs font-medium text-amber-400 hover:text-white underline-offset-2 hover:underline transition-colors"
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
        </>
      )}
    </div>
  );
});

export default PreemptionView;

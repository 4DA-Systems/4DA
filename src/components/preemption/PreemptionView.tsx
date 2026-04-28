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

function isStackAlert(item: EvidenceItem): boolean {
  return item.affected_deps.length > 0
    && (item.urgency === 'critical' || item.urgency === 'high');
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

  const { stackItems, ecosystemItems } = useMemo(() => {
    const visible = (feed?.items ?? [])
      .filter(item => !dismissedIds.has(item.id))
      .slice()
      .sort(
        (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
      );

    const stack: EvidenceItem[] = [];
    const ecosystem: EvidenceItem[] = [];
    for (const item of visible) {
      if (isStackAlert(item)) {
        stack.push(item);
      } else {
        ecosystem.push(item);
      }
    }
    return { stackItems: stack, ecosystemItems: ecosystem };
  }, [feed, dismissedIds]);

  const totalVisible = stackItems.length + ecosystemItems.length;

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
          <div className="w-12 h-12 rounded-full bg-green-500/10 border border-green-500/20 flex items-center justify-center mb-3">
            <span className="text-green-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm text-text-secondary">{t('preemption.empty')}</p>
        </div>
      )}

      {feed && totalVisible > 0 && (
        <>
          <div className="flex items-center gap-4 px-4 py-3 rounded-lg bg-bg-secondary border border-border">
            <div className="flex items-center gap-3 text-xs">
              {feed.critical_count > 0 && (
                <span className="inline-flex items-center gap-1.5 text-red-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
                  {feed.critical_count} critical
                </span>
              )}
              {feed.high_count > 0 && (
                <span className="inline-flex items-center gap-1.5 text-orange-400 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-orange-400" />
                  {feed.high_count} high
                </span>
              )}
            </div>
            <span className="ms-auto text-xs text-text-muted tabular-nums">
              {feed.total} {feed.total === 1 ? 'alert' : 'alerts'}
            </span>
          </div>

          {lastDismissed !== null && (
            <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20 animate-in fade-in">
              <span className="text-xs text-amber-400">Item dismissed</span>
              <button
                type="button"
                onClick={handleUndo}
                className="text-xs font-medium text-amber-400 hover:text-white underline-offset-2 hover:underline transition-colors"
              >
                Undo
              </button>
            </div>
          )}

          <PreemptionTierSection
            dotColor="#EF4444"
            borderColor="rgba(239, 68, 68, 0.2)"
            title="Your Stack"
            subtitle={`${stackItems.length} affecting your dependencies`}
            items={stackItems}
            surfacedRef={surfacedRef}
            onDismiss={handleDismiss}
            emptyText="No direct dependency alerts right now"
          />

          <PreemptionTierSection
            dotColor="#F59E0B"
            borderColor="rgba(245, 158, 11, 0.15)"
            title="Your Ecosystem"
            subtitle={`${ecosystemItems.length} ecosystem signals`}
            items={ecosystemItems}
            surfacedRef={surfacedRef}
            onDismiss={handleDismiss}
            emptyText="No ecosystem signals right now"
          />
        </>
      )}
    </div>
  );
});

export default PreemptionView;

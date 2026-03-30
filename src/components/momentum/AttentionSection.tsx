// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { AttentionItem } from './momentum-utils';

// ---------------------------------------------------------------------------
// Dismissed items (localStorage with 7-day TTL)
// ---------------------------------------------------------------------------

const DISMISSED_KEY = '4da_momentum_attention_dismissed';

function getDismissed(): Set<string> {
  try {
    const raw = localStorage.getItem(DISMISSED_KEY);
    if (raw === null || raw === '') return new Set();
    const data = JSON.parse(raw) as Record<string, number>;
    const now = Date.now();
    const valid: Record<string, number> = {};
    for (const [id, ts] of Object.entries(data)) {
      if (now - ts < 7 * 24 * 60 * 60 * 1000) valid[id] = ts;
    }
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(valid));
    return new Set(Object.keys(valid));
  } catch { return new Set(); }
}

function persistDismiss(id: string) {
  try {
    const raw = localStorage.getItem(DISMISSED_KEY);
    const data = (raw !== null && raw !== '') ? JSON.parse(raw) as Record<string, number> : {};
    data[id] = Date.now();
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(data));
  } catch { /* noop */ }
}

// ---------------------------------------------------------------------------
// Kind styling
// ---------------------------------------------------------------------------

const KIND_CONFIG: Record<string, { icon: string; accent: string; bg: string; border: string }> = {
  security:        { icon: '!',  accent: 'text-red-400',    bg: 'bg-red-500/10',    border: 'border-red-500/30' },
  decision_window: { icon: '?',  accent: 'text-amber-400',  bg: 'bg-amber-500/10',  border: 'border-amber-500/30' },
  knowledge_gap:   { icon: '~',  accent: 'text-purple-400', bg: 'bg-purple-500/10', border: 'border-purple-500/30' },
};

// ---------------------------------------------------------------------------
// Single attention card
// ---------------------------------------------------------------------------

const AttentionCard = memo(function AttentionCard({
  item,
  onView,
  onDismiss,
  index,
}: {
  item: AttentionItem;
  onView: () => void;
  onDismiss: () => void;
  index: number;
}) {
  const { t } = useTranslation();
  const { getTranslated } = useTranslatedContent();
  const cfg = KIND_CONFIG[item.kind] ?? KIND_CONFIG.security!;

  return (
    <div
      className={`rounded-lg border ${cfg.border} bg-bg-secondary p-4 flex items-start gap-3`}
      style={{ animation: `slideInRight 0.4s ease-out ${index * 60}ms both` }}
    >
      <div className={`w-7 h-7 rounded-lg ${cfg.bg} flex items-center justify-center flex-shrink-0 mt-0.5`}>
        <span className={`text-xs font-bold ${cfg.accent}`}>{cfg.icon}</span>
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm text-white font-medium leading-snug">{getTranslated(item.id, item.title)}</p>
        <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">{item.detail}</p>
      </div>
      <div className="flex items-center gap-1.5 flex-shrink-0">
        {item.entryName && (
          <button
            onClick={onView}
            className={`px-2.5 py-1 text-[11px] font-medium rounded-md transition-colors ${cfg.accent} ${cfg.bg} hover:opacity-80`}
          >
            {t('momentum.viewDetails')}
          </button>
        )}
        <button
          onClick={onDismiss}
          className="px-2 py-1 text-[11px] text-text-muted bg-bg-tertiary rounded-md hover:text-text-secondary transition-colors"
        >
          {t('momentum.dismiss')}
        </button>
      </div>
    </div>
  );
});

// ---------------------------------------------------------------------------
// All-clear state
// ---------------------------------------------------------------------------

function AllClear({ techCount }: { techCount: number }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-lg border border-border bg-bg-secondary p-5 text-center">
      <div className="w-8 h-8 mx-auto mb-2 rounded-full bg-green-500/10 flex items-center justify-center">
        <div className="w-2.5 h-2.5 rounded-full bg-green-400" />
      </div>
      <p className="text-sm text-text-secondary">
        {t('momentum.attentionClear')}
      </p>
      {techCount > 0 && (
        <p className="text-xs text-text-muted mt-1">
          {t('momentum.allClearDetail', { techCount })}
        </p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export interface AttentionSectionProps {
  items: AttentionItem[];
  techCount: number;
  onViewEntry: (name: string) => void;
  onActOnWindow: (windowId: number) => void;
  onCloseWindow: (windowId: number) => void;
}

export const AttentionSection = memo(function AttentionSection({
  items,
  techCount,
  onViewEntry,
  onActOnWindow,
  onCloseWindow,
}: AttentionSectionProps) {
  const { t } = useTranslation();
  const [dismissed, setDismissed] = useState<Set<string>>(() => getDismissed());

  const visibleItems = items.filter(item => !dismissed.has(item.id));

  const handleDismiss = useCallback((item: AttentionItem) => () => {
    if (item.windowId !== undefined) void onCloseWindow(item.windowId);
    persistDismiss(item.id);
    setDismissed(prev => new Set([...prev, item.id]));
  }, [onCloseWindow]);

  const handleView = useCallback((item: AttentionItem) => () => {
    if (item.entryName) {
      onViewEntry(item.entryName);
    } else if (item.windowId !== undefined) {
      void onActOnWindow(item.windowId);
    }
  }, [onViewEntry, onActOnWindow]);

  return (
    <section aria-label={t('momentum.attention')}>
      <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3 px-1">
        {t('momentum.attention')}
      </h3>
      {visibleItems.length === 0 ? (
        <AllClear techCount={techCount} />
      ) : (
        <div className="space-y-2">
          {visibleItems.map((item, i) => (
            <AttentionCard
              key={item.id}
              item={item}
              onView={handleView(item)}
              onDismiss={handleDismiss(item)}
              index={i}
            />
          ))}
        </div>
      )}
    </section>
  );
});

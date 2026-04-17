// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useRef, useState, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
import { recordTrustEvent } from '../../lib/trust-feedback';

// ============================================================================
// Constants
// ============================================================================

const URGENCY_CONFIG: Record<
  Urgency,
  { color: string; bg: string; border: string; dot: string; label: string }
> = {
  critical: {
    color: 'text-red-400',
    bg: 'bg-red-500/[0.06]',
    border: 'border-red-500/25',
    dot: 'bg-red-400',
    label: 'Critical',
  },
  high: {
    color: 'text-orange-400',
    bg: 'bg-orange-500/[0.05]',
    border: 'border-orange-500/25',
    dot: 'bg-orange-400',
    label: 'High',
  },
  medium: {
    color: 'text-yellow-400',
    bg: 'bg-yellow-500/[0.04]',
    border: 'border-yellow-500/20',
    dot: 'bg-yellow-400',
    label: 'Medium',
  },
  watch: {
    color: 'text-blue-400',
    bg: 'bg-blue-500/[0.04]',
    border: 'border-blue-500/20',
    dot: 'bg-blue-400',
    label: 'Watch',
  },
};

const URGENCY_ORDER: Urgency[] = ['critical', 'high', 'medium', 'watch'];

// Evidence items beyond this count are hidden behind a "Show all" toggle.
const EVIDENCE_COLLAPSE_THRESHOLD = 2;

// Maximum characters shown for an explanation before truncation.
const EXPLANATION_MAX_LENGTH = 280;

// ============================================================================
// Helpers
// ============================================================================

/** Format days-ago into a short human label. */
function formatFreshness(days: number): string {
  const d = Math.round(days);
  if (d <= 0) return 'today';
  if (d === 1) return 'yesterday';
  if (d < 7) return `${d}d ago`;
  if (d < 30) return `${Math.floor(d / 7)}w ago`;
  return `${Math.floor(d / 30)}mo ago`;
}

/** Truncate a string to N chars with ellipsis, at a word boundary. */
function truncateAt(text: string, limit: number): string {
  if (text.length <= limit) return text;
  const cut = text.slice(0, limit);
  const lastSpace = cut.lastIndexOf(' ');
  return `${lastSpace > limit - 40 ? cut.slice(0, lastSpace) : cut}…`;
}

/** Item `kind` is the canonical `EvidenceKind` variant; stringify it for
 * telemetry continuity with pre-Phase-3 data (where `alert_type` was used). */
function kindAsSourceType(item: EvidenceItem): string {
  return typeof item.kind === 'string' ? item.kind : String(item.kind);
}

/** Extract just the project directory name from a full path.
 * `C:\Users\Admin\Documents\kairos-mvp\backend` → `kairos-mvp/backend`
 * Shows the last 2 path segments for context without the full Windows path. */
function shortenProjectPath(fullPath: string): string {
  const parts = fullPath.replace(/\\/g, '/').split('/').filter(Boolean);
  if (parts.length <= 2) return parts.join('/');
  return parts.slice(-2).join('/');
}

/** Deduplicate shortened project names for display. */
function formatProjectNames(paths: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of paths) {
    const short = shortenProjectPath(p);
    if (!seen.has(short)) {
      seen.add(short);
      out.push(short);
    }
  }
  return out;
}

// ============================================================================
// Sub-components
// ============================================================================

const EvidenceList = memo(function EvidenceList({
  evidence,
  cardTitle,
}: {
  evidence: EvidenceItem['evidence'];
  cardTitle?: string;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  // Filter out citations whose title is identical to the card title —
  // no value in showing the same text twice.
  const filtered = cardTitle
    ? evidence.filter(e => e.title.toLowerCase() !== cardTitle.toLowerCase())
    : evidence;

  if (filtered.length === 0) return null;

  const shown = expanded ? filtered : filtered.slice(0, EVIDENCE_COLLAPSE_THRESHOLD);
  const canCollapse = filtered.length > EVIDENCE_COLLAPSE_THRESHOLD;

  return (
    <div className="mt-3 pt-3 border-t border-border/50">
      <h4 className="text-[10px] font-medium text-text-muted uppercase tracking-wider mb-2">
        {t('preemption.evidence')} ({evidence.length})
      </h4>
      <ul className="space-y-1.5">
        {shown.map((e, i) => (
          <li key={i} className="flex items-baseline gap-2 text-xs min-w-0">
            <span className="shrink-0 font-mono text-[10px] uppercase text-text-muted w-14 truncate">
              {e.source}
            </span>
            {e.url ? (
              <a
                href={e.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex-1 min-w-0 text-text-secondary hover:text-white transition-colors truncate"
                title={e.title}
              >
                {e.title}
              </a>
            ) : (
              <span
                className="flex-1 min-w-0 text-text-secondary truncate"
                title={e.title}
              >
                {e.title}
              </span>
            )}
            <span className="shrink-0 text-[10px] text-text-muted tabular-nums">
              {formatFreshness(e.freshness_days)}
            </span>
          </li>
        ))}
      </ul>
      {canCollapse && (
        <button
          type="button"
          onClick={() => { setExpanded(v => !v); }}
          className="mt-2 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
        >
          {expanded
            ? t('preemption.evidence.collapse', 'Show less')
            : `Show ${filtered.length - EVIDENCE_COLLAPSE_THRESHOLD} more`}
        </button>
      )}
    </div>
  );
});

const AffectedChips = memo(function AffectedChips({
  item,
}: {
  item: EvidenceItem;
}) {
  const { t } = useTranslation();
  const projectNames = formatProjectNames(item.affected_projects);
  const hasProjects = projectNames.length > 0;
  const hasDeps = item.affected_deps.length > 0;
  if (!hasProjects && !hasDeps) return null;

  return (
    <div className="mt-3 space-y-1.5 text-xs">
      {hasProjects && (
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="shrink-0 text-[10px] font-medium text-text-muted uppercase tracking-wider w-16">
            {t('preemption.affected.projects')}
          </span>
          <div className="flex flex-wrap gap-1">
            {projectNames.slice(0, 4).map((name) => (
              <span
                key={name}
                className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-bg-tertiary text-text-secondary border border-border"
              >
                {name}
              </span>
            ))}
            {projectNames.length > 4 && (
              <span className="text-[10px] text-text-muted">+{projectNames.length - 4}</span>
            )}
          </div>
        </div>
      )}
      {hasDeps && (
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="shrink-0 text-[10px] font-medium text-text-muted uppercase tracking-wider w-16">
            {t('preemption.affected.deps')}
          </span>
          <div className="flex flex-wrap gap-1">
            {item.affected_deps.slice(0, 6).map((dep) => (
              <span
                key={dep}
                className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-bg-tertiary text-text-secondary border border-border"
              >
                {dep}
              </span>
            ))}
            {item.affected_deps.length > 6 && (
              <span className="inline-flex items-center px-1.5 py-0.5 text-[10px] text-text-muted">
                +{item.affected_deps.length - 6}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
});

const lastClickRef = { current: 0 };

const ItemCard = memo(function ItemCard({
  item,
  surfacedRef,
  onDismiss,
}: {
  item: EvidenceItem;
  surfacedRef: React.RefObject<Set<string>>;
  onDismiss: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [explanationExpanded, setExplanationExpanded] = useState(false);
  const cfg = URGENCY_CONFIG[item.urgency] ?? URGENCY_CONFIG.watch;
  const sourceType = kindAsSourceType(item);

  // Record surfaced event once per item
  useEffect(() => {
    if (!surfacedRef.current!.has(item.id)) {
      surfacedRef.current!.add(item.id);
      recordTrustEvent({
        eventType: 'surfaced',
        alertId: item.id,
        sourceType,
        topic: item.title,
      });
    }
  }, [item.id, sourceType, item.title, surfacedRef]);

  const needsTruncation = item.explanation.length > EXPLANATION_MAX_LENGTH;
  const displayedExplanation = needsTruncation && !explanationExpanded
    ? truncateAt(item.explanation, EXPLANATION_MAX_LENGTH)
    : item.explanation;

  return (
    <article className={`rounded-lg border ${cfg.border} ${cfg.bg} overflow-hidden`}>
      {/* Header: urgency pill + title + confidence */}
      <header className="px-4 pt-4 pb-3">
        <div className="flex items-start gap-3">
          <span
            className={`shrink-0 inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider px-2 py-1 rounded ${cfg.color} bg-black/20 border ${cfg.border}`}
          >
            <span className={`w-1.5 h-1.5 rounded-full ${cfg.dot}`} />
            {cfg.label}
          </span>
          <h3 className="flex-1 min-w-0 text-[13px] font-medium text-white leading-snug">
            {item.title}
          </h3>
          <span
            className="shrink-0 text-[10px] font-mono tabular-nums text-text-muted"
            title={`Confidence provenance: ${item.confidence.provenance}${
              item.confidence.sample_size ? ` (n=${item.confidence.sample_size})` : ''
            }`}
          >
            {Math.round(item.confidence.value * 100)}%
          </span>
        </div>
      </header>

      {/* Body */}
      <div className="px-4 pb-4">
        {/* Explanation */}
        {item.explanation && (
          <p className="text-xs text-text-secondary leading-relaxed">
            {displayedExplanation}
            {needsTruncation && (
              <button
                type="button"
                onClick={() => { setExplanationExpanded(v => !v); }}
                className="ms-1 text-text-muted hover:text-text-secondary underline-offset-2 hover:underline"
              >
                {explanationExpanded
                  ? t('preemption.explanation.collapse', 'less')
                  : t('preemption.explanation.expand', 'more')}
              </button>
            )}
          </p>
        )}

        <AffectedChips item={item} />
        <EvidenceList evidence={item.evidence} cardTitle={item.title} />

        {/* Action buttons — each action_id maps to a real UX effect */}
        {item.suggested_actions.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2">
            {item.suggested_actions.map((action, i) => (
              <button
                key={i}
                type="button"
                className="px-3 py-1.5 text-[11px] rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-white hover:bg-bg-tertiary hover:border-white/20 transition-colors"
                title={action.description}
                onClick={() => {
                  recordTrustEvent({
                    eventType: action.action_id === 'dismiss' ? 'dismissed' : 'acted_on',
                    alertId: item.id,
                    sourceType,
                    topic: item.title,
                    notes: action.label,
                  });
                  if (action.action_id === 'dismiss' || action.action_id === 'snooze_7d') {
                    onDismiss(item.id);
                  } else if (action.action_id === 'investigate' || action.action_id === 'view_source') {
                    const now = Date.now();
                    if (now - lastClickRef.current < 500) return;
                    lastClickRef.current = now;
                    const url = item.evidence[0]?.url
                      ?? `https://www.google.com/search?q=${encodeURIComponent(item.title)}`;
                    import('@tauri-apps/plugin-opener')
                      .then(({ openUrl }) => openUrl(url))
                      .catch(() => window.open(url, '_blank', 'noopener,noreferrer'));
                  }
                }}
              >
                {action.label}
              </button>
            ))}
          </div>
        )}
      </div>
    </article>
  );
});

// ============================================================================
// Main View
// ============================================================================

const PreemptionView = memo(function PreemptionView() {
  const { t } = useTranslation();
  const surfacedRef = useRef(new Set<string>());
  // Persist dismissals in localStorage with 7-day TTL so they survive
  // page reloads. Without this, dismissed items reappear on refresh —
  // the #1 UX trust-breaker identified in edge case audit.
  const DISMISS_STORAGE_KEY = 'preemption_dismissed';
  const DISMISS_TTL_MS = 7 * 24 * 60 * 60 * 1000;
  const loadPersistedDismissals = (): Set<string> => {
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
  };
  const persistDismissal = (id: string) => {
    try {
      const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
      const parsed: Array<{ id: string; ts: number }> = raw ? JSON.parse(raw) : [];
      parsed.push({ id, ts: Date.now() });
      localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed));
    } catch { /* non-fatal */ }
  };
  const removeDismissal = (id: string) => {
    try {
      const raw = localStorage.getItem(DISMISS_STORAGE_KEY);
      if (!raw) return;
      const parsed: Array<{ id: string; ts: number }> = JSON.parse(raw);
      localStorage.setItem(DISMISS_STORAGE_KEY, JSON.stringify(parsed.filter(e => e.id !== id)));
    } catch { /* non-fatal */ }
  };

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

  // Sort items by urgency priority (critical first), filter dismissed
  const sortedItems = (feed?.items ?? [])
    .filter(item => !dismissedIds.has(item.id))
    .slice()
    .sort(
      (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
    );

  return (
    <div className="space-y-5" role="tabpanel" id="view-panel-preemption">
      {/* Header */}
      <header>
        <h1 className="text-xl font-semibold text-white tracking-tight">{t('preemption.title')}</h1>
        <p className="text-sm text-text-muted mt-1">{t('preemption.subtitle')}</p>
      </header>

      {/* Loading */}
      {loading && !feed && (
        <div className="flex items-center justify-center py-16">
          <p className="text-sm text-text-muted animate-pulse">{t('preemption.loading')}</p>
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-400">
          {error}
        </div>
      )}

      {/* Empty state */}
      {feed && sortedItems.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <div className="w-12 h-12 rounded-full bg-green-500/10 border border-green-500/20 flex items-center justify-center mb-3">
            <span className="text-green-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm text-text-secondary">{t('preemption.empty')}</p>
        </div>
      )}

      {/* Alert list */}
      {feed && sortedItems.length > 0 && (
        <>
          {/* Summary bar */}
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

          {/* Undo bar — shows for 8s after a dismiss */}
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

          {/* Item cards */}
          <div className="space-y-4">
            {sortedItems.map(item => (
              <ItemCard key={item.id} item={item} surfacedRef={surfacedRef} onDismiss={handleDismiss} />
            ))}
          </div>
        </>
      )}
    </div>
  );
});

export default PreemptionView;

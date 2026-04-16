// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useRef, useState, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { PreemptionAlert, PreemptionUrgency } from '../../store/preemption-slice';
import { recordTrustEvent } from '../../lib/trust-feedback';

// ============================================================================
// Constants
// ============================================================================

const URGENCY_CONFIG: Record<
  PreemptionUrgency,
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

const URGENCY_ORDER: PreemptionUrgency[] = ['critical', 'high', 'medium', 'watch'];

// Evidence items beyond this count are hidden behind a "Show all" toggle.
const EVIDENCE_COLLAPSE_THRESHOLD = 2;

// Maximum characters shown for an explanation before truncation.
const EXPLANATION_MAX_LENGTH = 280;

// ============================================================================
// Helpers
// ============================================================================

/** Format days-ago into a short human label. */
function formatFreshness(days: number): string {
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 7) return `${days}d ago`;
  if (days < 30) return `${Math.floor(days / 7)}w ago`;
  return `${Math.floor(days / 30)}mo ago`;
}

/** Truncate a string to N chars with ellipsis, at a word boundary. */
function truncateAt(text: string, limit: number): string {
  if (text.length <= limit) return text;
  const cut = text.slice(0, limit);
  const lastSpace = cut.lastIndexOf(' ');
  return `${lastSpace > limit - 40 ? cut.slice(0, lastSpace) : cut}…`;
}

// ============================================================================
// Sub-components
// ============================================================================

const EvidenceList = memo(function EvidenceList({
  evidence,
}: {
  evidence: PreemptionAlert['evidence'];
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  if (evidence.length === 0) return null;

  const shown = expanded ? evidence : evidence.slice(0, EVIDENCE_COLLAPSE_THRESHOLD);
  const hiddenCount = evidence.length - EVIDENCE_COLLAPSE_THRESHOLD;
  const canCollapse = evidence.length > EVIDENCE_COLLAPSE_THRESHOLD;

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
            : t('preemption.evidence.expand', `Show ${hiddenCount} more`, { count: hiddenCount })}
        </button>
      )}
    </div>
  );
});

const AffectedChips = memo(function AffectedChips({
  alert,
}: {
  alert: PreemptionAlert;
}) {
  const { t } = useTranslation();
  const hasProjects = alert.affected_projects.length > 0;
  const hasDeps = alert.affected_dependencies.length > 0;
  if (!hasProjects && !hasDeps) return null;

  return (
    <div className="mt-3 space-y-1.5 text-xs">
      {hasProjects && (
        <div className="flex items-baseline gap-2">
          <span className="shrink-0 text-[10px] font-medium text-text-muted uppercase tracking-wider w-16">
            {t('preemption.affected.projects')}
          </span>
          <span className="text-text-secondary truncate" title={alert.affected_projects.join(', ')}>
            {alert.affected_projects.join(', ')}
          </span>
        </div>
      )}
      {hasDeps && (
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="shrink-0 text-[10px] font-medium text-text-muted uppercase tracking-wider w-16">
            {t('preemption.affected.deps')}
          </span>
          <div className="flex flex-wrap gap-1">
            {alert.affected_dependencies.slice(0, 6).map((dep) => (
              <span
                key={dep}
                className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-bg-tertiary text-text-secondary border border-border"
              >
                {dep}
              </span>
            ))}
            {alert.affected_dependencies.length > 6 && (
              <span className="inline-flex items-center px-1.5 py-0.5 text-[10px] text-text-muted">
                +{alert.affected_dependencies.length - 6}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
});

const AlertCard = memo(function AlertCard({
  alert,
  surfacedRef,
}: {
  alert: PreemptionAlert;
  surfacedRef: React.RefObject<Set<string>>;
}) {
  const { t } = useTranslation();
  const [explanationExpanded, setExplanationExpanded] = useState(false);
  const cfg = URGENCY_CONFIG[alert.urgency] ?? URGENCY_CONFIG.watch;

  // Record surfaced event once per alert
  useEffect(() => {
    if (!surfacedRef.current!.has(alert.id)) {
      surfacedRef.current!.add(alert.id);
      recordTrustEvent({
        eventType: 'surfaced',
        alertId: alert.id,
        sourceType: alert.alert_type,
        topic: alert.title,
      });
    }
  }, [alert.id, alert.alert_type, alert.title, surfacedRef]);

  const needsTruncation = alert.explanation.length > EXPLANATION_MAX_LENGTH;
  const displayedExplanation = needsTruncation && !explanationExpanded
    ? truncateAt(alert.explanation, EXPLANATION_MAX_LENGTH)
    : alert.explanation;

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
            {alert.title}
          </h3>
          <span className="shrink-0 text-[10px] font-mono tabular-nums text-text-muted">
            {Math.round(alert.confidence * 100)}%
          </span>
        </div>
      </header>

      {/* Body */}
      <div className="px-4 pb-4">
        {/* Explanation */}
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

        <AffectedChips alert={alert} />
        <EvidenceList evidence={alert.evidence} />

        {/* Predicted window */}
        {alert.predicted_window && (
          <div className="mt-3 text-[11px] text-text-muted">
            <span className="text-text-muted">{t('preemption.window')}:</span>{' '}
            <span className="text-text-secondary">{alert.predicted_window}</span>
          </div>
        )}

        {/* Action buttons */}
        {alert.suggested_actions.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-2">
            {alert.suggested_actions.map((action, i) => (
              <button
                key={i}
                type="button"
                className="px-3 py-1.5 text-[11px] rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-white hover:bg-bg-tertiary hover:border-white/20 transition-colors"
                title={action.description}
                onClick={() => {
                  recordTrustEvent({
                    eventType: action.label.toLowerCase().includes('dismiss')
                      ? 'dismissed'
                      : 'acted_on',
                    alertId: alert.id,
                    sourceType: alert.alert_type,
                    topic: alert.title,
                    notes: action.label,
                  });
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

  // Sort alerts by urgency priority (critical first)
  const sortedAlerts = (feed?.alerts ?? []).slice().sort(
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
      {feed && sortedAlerts.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <div className="w-12 h-12 rounded-full bg-green-500/10 border border-green-500/20 flex items-center justify-center mb-3">
            <span className="text-green-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm text-text-secondary">{t('preemption.empty')}</p>
        </div>
      )}

      {/* Alert list */}
      {feed && sortedAlerts.length > 0 && (
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

          {/* Alert cards */}
          <div className="space-y-4">
            {sortedAlerts.map(alert => (
              <AlertCard key={alert.id} alert={alert} surfacedRef={surfacedRef} />
            ))}
          </div>
        </>
      )}
    </div>
  );
});

export default PreemptionView;

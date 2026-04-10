// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useRef, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import type { PreemptionAlert, PreemptionUrgency } from '../../store/preemption-slice';
import { recordTrustEvent } from '../../lib/trust-feedback';

// ============================================================================
// Constants
// ============================================================================

const URGENCY_CONFIG: Record<PreemptionUrgency, { color: string; bg: string; border: string; label: string }> = {
  critical: { color: 'text-red-400', bg: 'bg-red-500/10', border: 'border-red-500/30', label: 'Critical' },
  high: { color: 'text-orange-400', bg: 'bg-orange-500/10', border: 'border-orange-500/30', label: 'High' },
  medium: { color: 'text-yellow-400', bg: 'bg-yellow-500/10', border: 'border-yellow-500/30', label: 'Medium' },
  watch: { color: 'text-blue-400', bg: 'bg-blue-500/10', border: 'border-blue-500/30', label: 'Watch' },
};

const URGENCY_ORDER: PreemptionUrgency[] = ['critical', 'high', 'medium', 'watch'];

// ============================================================================
// Sub-components
// ============================================================================

const EvidenceList = memo(function EvidenceList({ evidence }: { evidence: PreemptionAlert['evidence'] }) {
  const { t } = useTranslation();
  if (evidence.length === 0) return null;

  return (
    <div className="mt-3">
      <h4 className="text-xs font-medium text-text-secondary mb-1.5">{t('preemption.evidence')}</h4>
      <div className="space-y-1">
        {evidence.map((e, i) => (
          <div key={i} className="flex items-baseline gap-2 text-xs">
            <span className="text-text-muted shrink-0">{e.source}:</span>
            {e.url ? (
              <a
                href={e.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-text-secondary hover:text-white transition-colors truncate"
              >
                {e.title}
              </a>
            ) : (
              <span className="text-text-secondary truncate">{e.title}</span>
            )}
            <span className="text-text-muted shrink-0 ms-auto">
              {e.freshness_days === 0 ? 'today' : `${e.freshness_days}d ago`}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
});

const AlertCard = memo(function AlertCard({ alert, surfacedRef }: { alert: PreemptionAlert; surfacedRef: React.RefObject<Set<string>> }) {
  const { t } = useTranslation();
  const cfg = URGENCY_CONFIG[alert.urgency] ?? URGENCY_CONFIG.watch;

  // Record surfaced event once per alert
  useEffect(() => {
    if (!surfacedRef.current!.has(alert.id)) {
      surfacedRef.current!.add(alert.id);
      recordTrustEvent({ eventType: 'surfaced', alertId: alert.id, sourceType: alert.alert_type, topic: alert.title });
    }
  }, [alert.id, alert.alert_type, alert.title, surfacedRef]);

  return (
    <div className={`rounded-lg border ${cfg.border} ${cfg.bg} p-4`}>
      {/* Header */}
      <div className="flex items-start gap-3">
        <span className={`shrink-0 text-[10px] font-semibold uppercase px-2 py-0.5 rounded ${cfg.color} ${cfg.bg} border ${cfg.border}`}>
          {cfg.label}
        </span>
        <h3 className="text-sm font-medium text-white leading-snug">{alert.title}</h3>
      </div>

      {/* Explanation */}
      <p className="mt-2 text-xs text-text-secondary leading-relaxed">{alert.explanation}</p>

      {/* Evidence */}
      <EvidenceList evidence={alert.evidence} />

      {/* Affected projects & deps */}
      <div className="mt-3 flex flex-wrap gap-x-6 gap-y-1.5 text-xs">
        {alert.affected_projects.length > 0 && (
          <div>
            <span className="text-text-muted">{t('preemption.affected.projects')}: </span>
            <span className="text-text-secondary">{alert.affected_projects.join(', ')}</span>
          </div>
        )}
        {alert.affected_dependencies.length > 0 && (
          <div>
            <span className="text-text-muted">{t('preemption.affected.deps')}: </span>
            <span className="text-text-secondary">{alert.affected_dependencies.join(', ')}</span>
          </div>
        )}
      </div>

      {/* Metadata row: confidence + predicted window */}
      <div className="mt-3 flex items-center gap-4 text-[11px] text-text-muted">
        <span>{t('preemption.confidence', { value: Math.round(alert.confidence * 100) })}</span>
        {alert.predicted_window && (
          <span>{t('preemption.window')}: {alert.predicted_window}</span>
        )}
      </div>

      {/* Action buttons */}
      {alert.suggested_actions.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {alert.suggested_actions.map((action, i) => (
            <button
              key={i}
              className="px-3 py-1 text-xs rounded-md border border-border bg-bg-tertiary text-text-secondary hover:text-white hover:border-white/20 transition-colors"
              title={action.description}
              onClick={() => {
                recordTrustEvent({
                  eventType: action.label.toLowerCase().includes('dismiss') ? 'dismissed' : 'acted_on',
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

  // Sort alerts by urgency priority
  const sortedAlerts = (feed?.alerts ?? []).slice().sort(
    (a, b) => URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency),
  );

  return (
    <div className="space-y-4" role="tabpanel" id="view-panel-preemption">
      {/* Header */}
      <div>
        <h1 className="text-lg font-semibold text-white">{t('preemption.title')}</h1>
        <p className="text-xs text-text-muted mt-0.5">{t('preemption.subtitle')}</p>
      </div>

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
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-12 h-12 rounded-full bg-green-500/10 border border-green-500/20 flex items-center justify-center mb-3">
            <span className="text-green-400 text-lg">&#x2713;</span>
          </div>
          <p className="text-sm text-text-secondary">{t('preemption.empty')}</p>
        </div>
      )}

      {/* Summary bar */}
      {feed && sortedAlerts.length > 0 && (
        <>
          <div className="flex items-center gap-3 px-4 py-2.5 rounded-lg bg-bg-secondary border border-border text-xs">
            {feed.critical_count > 0 && (
              <span className="text-red-400 font-medium">{feed.critical_count} critical</span>
            )}
            {feed.high_count > 0 && (
              <>
                {feed.critical_count > 0 && <span className="text-text-muted">·</span>}
                <span className="text-orange-400 font-medium">{feed.high_count} high</span>
              </>
            )}
            <span className="text-text-muted">·</span>
            <span className="text-text-secondary">
              {t('preemption.summary', {
                critical: feed.critical_count,
                high: feed.high_count,
                total: feed.total,
              })}
            </span>
          </div>

          {/* Alert cards */}
          <div className="space-y-3">
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

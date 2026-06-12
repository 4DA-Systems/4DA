// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SourceRelevance } from '../types';
import { useLicense } from '../hooks/use-license';
import { SignalUpgradeCTA } from './SignalUpgradeCTA';
import { SIGNAL_CONFIG, PRIORITY_CONFIG, SIGNAL_LABELS, LANES } from './signals/signal-config';
import { SignalRow } from './signals/SignalRow';
import type { SignalItem } from './signals/SignalRow';

// ============================================================================
// Types
// ============================================================================

interface SignalsPanelProps {
  results: SourceRelevance[];
}

// ============================================================================
// Component
// ============================================================================

export const SignalsPanel = memo(function SignalsPanel({ results }: SignalsPanelProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(true);
  const [typeFilter, setTypeFilter] = useState<string | null>(null);
  const [priorityFilter, setPriorityFilter] = useState<string | null>(null);
  const { isPro } = useLicense();

  const { signals, filtered, typeCounts, priorityCounts, lanes } = useMemo(() => {
    const signals: SignalItem[] = results
      .filter((r) => r.signal_type && r.signal_priority && r.signal_action)
      .map((r) => ({
        id: r.id,
        title: r.title,
        url: r.url,
        top_score: r.top_score,
        source_type: r.source_type || 'unknown',
        signal_type: r.signal_type!,
        signal_priority: r.signal_priority!,
        signal_action: r.signal_action!,
        signal_triggers: r.signal_triggers || [],
        similar_count: r.similar_count || 0,
        similar_titles: r.similar_titles || [],
      }));

    const priorityOrder: Record<string, number> = { critical: 4, alert: 3, advisory: 2, watch: 1 };
    const sorted = [...signals].sort((a, b) => {
      const pd = (priorityOrder[b.signal_priority] || 0) - (priorityOrder[a.signal_priority] || 0);
      if (pd !== 0) return pd;
      return b.top_score - a.top_score;
    });

    const filtered = sorted
      .filter((s) => !typeFilter || s.signal_type === typeFilter)
      .filter((s) => !priorityFilter || s.signal_priority === priorityFilter);

    const typeCounts: Record<string, number> = {};
    const priorityCounts: Record<string, number> = {};
    for (const s of signals) {
      typeCounts[s.signal_type] = (typeCounts[s.signal_type] || 0) + 1;
      priorityCounts[s.signal_priority] = (priorityCounts[s.signal_priority] || 0) + 1;
    }

    // Group filtered signals into lanes. Each signal goes into the first
    // matching lane; "Critical Now" requires both type AND priority match.
    const claimed = new Set<number>();
    const lanes = LANES.map((lane) => {
      const items = filtered.filter((s) => {
        if (claimed.has(s.id)) return false;
        if (!lane.types.has(s.signal_type)) return false;
        if (lane.priorityFilter && !lane.priorityFilter.has(s.signal_priority)) return false;
        return true;
      });
      for (const item of items) claimed.add(item.id);
      return { config: lane, items };
    });

    return { signals, sorted, filtered, typeCounts, priorityCounts, lanes };
  }, [results, typeFilter, priorityFilter]);

  if (signals.length === 0) return (
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border px-5 py-4">
      <p className="text-text-muted text-sm text-center">{t('signals.noSignals')}</p>
    </div>
  );

  const criticalCount = priorityCounts['critical'] || 0;
  const highCount = priorityCounts['alert'] || 0;

  return (
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-label={t('signals.title')}
        className="w-full px-5 py-4 border-b border-border flex items-center justify-between hover:bg-[#1A1A1A] transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center" aria-hidden="true">
            <span className="text-text-secondary">⚡</span>
          </div>
          <div className="text-start">
            <h2 className="font-medium text-text-primary">{t('signals.title')}</h2>
            <p className="text-xs text-text-muted">
              {t('signals.actionable', { count: signals.length })}
              {criticalCount > 0 && (
                <span className="ms-2 text-red-400">{t('signals.critical', { count: criticalCount })}</span>
              )}
              {highCount > 0 && (
                <span className="ms-2 text-orange-400">{t('signals.high', { count: highCount })}</span>
              )}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          {/* Priority dots summary */}
          <div className="flex gap-1" aria-hidden="true">
            {criticalCount > 0 && <span className="w-2 h-2 rounded-full bg-red-400" />}
            {highCount > 0 && <span className="w-2 h-2 rounded-full bg-orange-400" />}
            {(priorityCounts['advisory'] || 0) > 0 && <span className="w-2 h-2 rounded-full bg-yellow-400" />}
          </div>
          <span className="text-text-muted text-sm" aria-hidden="true">{expanded ? '▾' : '▸'}</span>
        </div>
      </button>

      {/* Free tier: category overview + upgrade prompt */}
      {expanded && !isPro && (
        <div className="p-4 space-y-4">
          <div className="flex flex-wrap gap-2">
            {Object.entries(typeCounts).map(([type, count]) => {
              const config = SIGNAL_CONFIG[type];
              return (
                <span key={type} className="px-2.5 py-1 text-[11px] rounded-lg border bg-bg-tertiary text-text-muted border-border flex items-center gap-1.5">
                  <span>{config?.icon ?? '?'}</span>
                  <span>{SIGNAL_LABELS[type] ?? type}</span>
                  <span className="text-[10px] opacity-60">{count}</span>
                </span>
              );
            })}
            <span className="self-center text-border">|</span>
            {['critical', 'alert', 'advisory'].map((p) => {
              const count = priorityCounts[p] || 0;
              if (count === 0) return null;
              const config = PRIORITY_CONFIG[p]!;
              return (
                <span key={p} className="px-2 py-1 text-[10px] rounded-lg border bg-bg-tertiary text-text-muted border-border flex items-center gap-1.5">
                  <span className={`w-1.5 h-1.5 rounded-full ${config.dot}`} />
                  <span>{config.label}</span>
                  <span className="opacity-60">{count}</span>
                </span>
              );
            })}
          </div>
          <div className="text-center py-2 space-y-3">
            <p className="text-sm text-text-secondary">
              {t('signals.freeTeaser', {
                count: signals.length,
              })}
            </p>
            <p className="text-xs text-text-muted">
              {t('signals.freeSubtext')}
            </p>
            <SignalUpgradeCTA compact />
          </div>
        </div>
      )}

      {/* Pro tier: full interactive filters + signal items */}
      {expanded && isPro && (
        <div className="p-4">
          {/* Filters */}
          <div className="flex flex-wrap gap-2 mb-4">
            {/* Type filters */}
            {Object.entries(typeCounts).map(([type, count]) => {
              const config = SIGNAL_CONFIG[type];
              const isActive = typeFilter === type;
              return (
                <button
                  key={type}
                  onClick={() => setTypeFilter(isActive ? null : type)}
                  aria-label={`Filter by signal type: ${SIGNAL_LABELS[type] ?? type}`}
                  aria-pressed={isActive}
                  className={`px-2.5 py-1 text-[11px] rounded-lg border transition-all flex items-center gap-1.5 ${
                    isActive
                      ? `${config?.bgColor ?? 'bg-text-primary/10'} ${config?.color ?? 'text-text-primary'} ${config?.borderColor ?? 'border-text-primary/20'}`
                      : 'bg-bg-tertiary text-text-secondary border-border hover:border-[#3A3A3A]'
                  }`}
                >
                  <span>{config?.icon ?? '?'}</span>
                  <span>{SIGNAL_LABELS[type] ?? type}</span>
                  <span className="text-[10px] opacity-60">{count}</span>
                </button>
              );
            })}

            {/* Divider */}
            {Object.keys(typeCounts).length > 0 && (
              <span className="self-center text-border">|</span>
            )}

            {/* Priority filters */}
            {['critical', 'alert', 'advisory', 'watch'].map((p) => {
              const count = priorityCounts[p] || 0;
              if (count === 0) return null;
              const config = PRIORITY_CONFIG[p]!;
              const isActive = priorityFilter === p;
              return (
                <button
                  key={p}
                  onClick={() => setPriorityFilter(isActive ? null : p)}
                  aria-label={`Filter by priority: ${config.label}`}
                  aria-pressed={isActive}
                  className={`px-2 py-1 text-[10px] font-medium rounded-lg border transition-all flex items-center gap-1.5 ${
                    isActive
                      ? `${config.bgColor} ${config.color} border-current`
                      : 'bg-bg-tertiary text-text-muted border-border hover:border-[#3A3A3A]'
                  }`}
                >
                  <span className={`w-1.5 h-1.5 rounded-full ${config.dot}`} />
                  <span>{config.label}</span>
                  <span className="opacity-60">{count}</span>
                </button>
              );
            })}

            {/* Clear filters */}
            {(typeFilter || priorityFilter) && (
              <button
                onClick={() => { setTypeFilter(null); setPriorityFilter(null); }}
                aria-label={t('signals.clear')}
                className="px-2 py-1 text-[10px] text-text-muted hover:text-text-primary transition-colors"
              >
                {t('signals.clear')}
              </button>
            )}
          </div>

          {/* Signal Items — grouped by lane */}
          <div className="space-y-4 max-h-[500px] overflow-y-auto" role="list" aria-label={t('signals.title')}>
            {filtered.length === 0 ? (
              <p className="text-center text-sm text-text-muted py-4">{t('signals.noMatch')}</p>
            ) : (typeFilter || priorityFilter) ? (
              /* When filters are active, show flat list (user is already narrowing) */
              <div className="space-y-2">
                {filtered.map((signal) => (
                  <SignalRow key={signal.id} signal={signal} />
                ))}
              </div>
            ) : (
              /* Unfiltered: show grouped lanes */
              lanes.filter((l) => l.items.length > 0).map((lane) => (
                <div key={lane.config.key}>
                  <div className={`flex items-center gap-2 mb-2 pb-1 border-b ${lane.config.borderColor}`}>
                    <span className="text-xs" aria-hidden="true">{lane.config.icon}</span>
                    <span className={`text-xs font-medium ${lane.config.color}`}>{lane.config.label}</span>
                    <span className="text-[10px] text-text-muted">{lane.items.length}</span>
                  </div>
                  <div className="space-y-2">
                    {lane.items.map((signal) => (
                      <SignalRow key={signal.id} signal={signal} />
                    ))}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
});

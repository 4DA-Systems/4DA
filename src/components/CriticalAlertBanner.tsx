// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranslatedContent } from './ContentTranslationProvider';
import { useAppStore } from '../store';
import type { SourceRelevance } from '../types/analysis';

interface CriticalAlert {
  id: number;
  title: string;
  signal_action?: string;
  signal_type?: string;
  source_type?: string;
  url?: string;
}

/**
 * Persistent critical alert banner — shows when security advisories
 * or breaking changes affect the user's actual dependencies.
 *
 * Unlike HealthBanner (dismissible, one-time), this banner persists
 * until the user explicitly acknowledges each alert. It survives
 * navigation between views.
 */
export function CriticalAlertBanner() {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  const relevanceResults = useAppStore(s => s.appState.relevanceResults);
  const [acknowledged, setAcknowledged] = useState<Set<number>>(
    () => {
      try {
        const stored = localStorage.getItem('4da-acknowledged-alerts');
        return stored != null ? new Set(JSON.parse(stored) as number[]) : new Set<number>();
      } catch {
        return new Set<number>();
      }
    },
  );

  // Find critical alerts from results
  const criticalAlerts: CriticalAlert[] = useMemo(() =>
    relevanceResults
      .filter((r: SourceRelevance) =>
        r.signal_priority === 'critical'
        && (r.signal_action?.startsWith('Critical:') ?? false)
        && !acknowledged.has(r.id),
      )
      .map((r: SourceRelevance) => ({
        id: r.id,
        title: r.title,
        signal_action: r.signal_action,
        signal_type: r.signal_type,
        source_type: r.source_type,
        url: r.url ?? undefined,
      }))
      .slice(0, 3),
    [relevanceResults, acknowledged],
  );

  const handleAcknowledge = useCallback((id: number) => {
    setAcknowledged(prev => {
      const next = new Set(prev);
      next.add(id);
      try {
        localStorage.setItem('4da-acknowledged-alerts', JSON.stringify([...next]));
      } catch { /* localStorage full */ }
      return next;
    });
  }, []);

  const handleAcknowledgeAll = useCallback(() => {
    setAcknowledged(prev => {
      const next = new Set(prev);
      for (const alert of criticalAlerts) {
        next.add(alert.id);
      }
      try {
        localStorage.setItem('4da-acknowledged-alerts', JSON.stringify([...next]));
      } catch { /* localStorage full */ }
      return next;
    });
  }, [criticalAlerts]);

  // Request translations for critical alert content
  useEffect(() => {
    if (criticalAlerts.length > 0) {
      requestTranslation(criticalAlerts.flatMap(a => {
        const items = [{ id: `alert-title-${a.id}`, text: a.title }];
        if (a.signal_action) items.push({ id: `alert-action-${a.id}`, text: a.signal_action });
        return items;
      }));
    }
  }, [criticalAlerts, requestTranslation]);

  // Trigger browser notification for new critical alerts when app is hidden
  useEffect(() => {
    if (criticalAlerts.length === 0 || !document.hidden) return;
    const first = criticalAlerts[0];
    if (first == null) return;
    if ('Notification' in window && Notification.permission === 'granted') {
      void new Notification('4DA — Attention', {
        body: first.signal_action ?? first.title,
        tag: `4da-critical-${String(first.id)}`,
        requireInteraction: true,
      });
    }
  }, [criticalAlerts]);

  if (criticalAlerts.length === 0) return null;

  return (
    <div className="mx-4 mt-2 mb-1 bg-amber-500/8 border border-amber-500/30 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-amber-400" />
          <span className="text-sm font-medium text-amber-400">
            {criticalAlerts.length === 1
              ? t('alerts.criticalSingular', 'Needs Attention')
              : t('alerts.criticalPlural', '{{count}} Items Need Attention', { count: criticalAlerts.length })}
          </span>
        </div>
        {criticalAlerts.length > 1 && (
          <button
            onClick={handleAcknowledgeAll}
            className="text-xs text-text-muted hover:text-text-secondary transition-colors"
          >
            {t('alerts.acknowledgeAll', 'Acknowledge all')}
          </button>
        )}
      </div>

      {/* Alert list */}
      <div className="px-3 pb-2 space-y-1.5">
        {criticalAlerts.map(alert => (
          <div
            key={alert.id}
            className="flex items-start justify-between gap-2 text-xs bg-amber-500/5 rounded px-2 py-1.5"
          >
            <div className="flex-1 min-w-0">
              <div className="text-amber-200 font-medium truncate">
                {alert.signal_action != null && alert.signal_action !== ''
                  ? getTranslated(`alert-action-${alert.id}`, alert.signal_action)
                  : getTranslated(`alert-title-${alert.id}`, alert.title)}
              </div>
              {alert.signal_action != null && alert.signal_action !== '' && (
                <div className="text-text-muted truncate mt-0.5">
                  {getTranslated(`alert-title-${alert.id}`, alert.title)}
                </div>
              )}
            </div>
            <div className="flex items-center gap-1.5 shrink-0">
              {alert.url != null && alert.url !== '' && (
                <button
                  onClick={() => {
                    import('@tauri-apps/plugin-opener').then(({ openUrl }) => {
                      void openUrl(alert.url!);
                    }).catch(() => {
                      window.open(alert.url, '_blank', 'noopener,noreferrer');
                    });
                  }}
                  className="text-amber-400 hover:text-amber-300 transition-colors underline cursor-pointer"
                >
                  {t('alerts.details', 'Details')}
                </button>
              )}
              <button
                onClick={() => { handleAcknowledge(alert.id); }}
                className="text-text-muted hover:text-text-secondary transition-colors"
                title={t('alerts.acknowledge', 'Acknowledge')}
              >
                {'\u2713'}
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

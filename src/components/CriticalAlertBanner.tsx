// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranslatedContent } from './ContentTranslationProvider';
import { useAppStore } from '../store';
import { DISMISSAL_DAYS, HIDING_ACTIONS, dismissAlert, undoDismissal } from '../lib/alert-dismissals';
import { cmd } from '../lib/commands';
import { readableError } from '../utils/error-messages';
import { isSafeUrl } from '../utils/sanitize-html';
import type { SourceRelevance } from '../types/analysis';

interface CriticalAlert {
  id: number;
  title: string;
  signal_action?: string;
  signal_type?: string;
  source_type?: string;
  url?: string;
  advisory_id?: string;
  applicability?: string;
  cvss_score?: number;
  cvss_severity?: string;
  matched_deps?: string[];
  fixed_version?: string;
  installed_version?: string;
  dependency_path?: string;
}

/** Alerts listed at once; the header counts all of them. */
const SHOWN = 3;

/**
 * Persistent critical alert banner — shows when security advisories
 * or breaking changes affect the user's actual dependencies.
 *
 * It persists until the user triages each alert. Triage goes to the one
 * dismissal store Preemption also uses (lib/alert-dismissals): every
 * dismissal expires after DISMISSAL_DAYS and can be undone. "Investigate"
 * only opens the advisory; it never hides the alert (audit 2026-11-11).
 */
export function CriticalAlertBanner() {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  const relevanceResults = useAppStore(s => s.appState.relevanceResults);
  const [triaged, setTriaged] = useState<Set<number>>(new Set<number>());
  // The alerts the last dismissal hid, for Undo.
  const [lastDismissed, setLastDismissed] = useState<CriticalAlert[]>([]);
  const [triageError, setTriageError] = useState<string | null>(null);

  // Load live triage decisions when results change. A failed read hides
  // nothing: an alert shows until a decision about it is known.
  useEffect(() => {
    const ids = relevanceResults
      .filter((r: SourceRelevance) => r.is_critical_alert === true)
      .map((r: SourceRelevance) => r.id);
    if (ids.length === 0) return;

    void cmd('get_triage_states', { itemIds: ids })
      .then(records => {
        setTriaged(new Set(records.filter(r => HIDING_ACTIONS.has(r.action)).map(r => r.item_id)));
      })
      .catch(() => { /* show every alert */ });
  }, [relevanceResults]);

  // Every untriaged critical alert: the header counts these and the list
  // shows the first SHOWN (it used to slice first and count the slice).
  const pendingAlerts: CriticalAlert[] = useMemo(() =>
    relevanceResults
      .filter((r: SourceRelevance) =>
        r.is_critical_alert === true
        && !triaged.has(r.id),
      )
      .map((r: SourceRelevance) => ({
        id: r.id,
        title: r.title,
        signal_action: r.signal_action,
        signal_type: r.signal_type,
        source_type: r.source_type,
        url: r.url ?? undefined,
        advisory_id: r.advisory_id,
        applicability: r.applicability,
        cvss_score: r.score_breakdown?.cvss_score,
        cvss_severity: r.score_breakdown?.cvss_severity,
        matched_deps: r.score_breakdown?.matched_deps,
        fixed_version: r.score_breakdown?.fixed_version,
        installed_version: r.score_breakdown?.installed_version,
        dependency_path: r.score_breakdown?.dependency_path,
      })),
    [relevanceResults, triaged],
  );
  const criticalAlerts = useMemo(() => pendingAlerts.slice(0, SHOWN), [pendingAlerts]);

  // Hide at once, store it, and put it back if the store refused: a
  // dismissal that was not saved must not look saved.
  const handleTriage = useCallback((alerts: CriticalAlert[], action: string) => {
    const ids = alerts.map(a => a.id);
    setTriageError(null);
    setTriaged(prev => new Set([...prev, ...ids]));
    setLastDismissed(alerts);
    void Promise.all(alerts.map(a => dismissAlert({ itemId: a.id }, action, a.advisory_id ?? null)))
      .catch((e: unknown) => {
        setTriaged(prev => new Set([...prev].filter(id => !ids.includes(id))));
        setLastDismissed([]);
        setTriageError(readableError(e));
      });
  }, []);

  const handleUndo = useCallback(() => {
    const alerts = lastDismissed;
    setLastDismissed([]);
    setTriaged(prev => new Set([...prev].filter(id => !alerts.some(a => a.id === id))));
    void Promise.all(alerts.map(a => undoDismissal({ itemId: a.id })))
      .catch((e: unknown) => setTriageError(readableError(e)));
  }, [lastDismissed]);

  const handleTriageAll = useCallback(() => {
    handleTriage(pendingAlerts, 'acknowledged');
  }, [pendingAlerts, handleTriage]);

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

  const notices = (
    <>
      {triageError && (
        <div role="alert" className="mx-4 mt-2 mb-1 px-3 py-2 text-xs bg-red-900/20 border border-red-500/30 rounded-lg text-red-300">
          {t('alerts.triageFailed', { error: triageError })}
        </div>
      )}
      {lastDismissed.length > 0 && (
        <div role="status" className="mx-4 mt-2 mb-1 px-3 py-2 flex items-center gap-2 text-xs bg-bg-secondary border border-border rounded-lg text-text-secondary">
          <span className="flex-1">{t('alerts.dismissedFor', { count: lastDismissed.length, days: DISMISSAL_DAYS })}</span>
          <button onClick={handleUndo} className="px-2 py-0.5 rounded bg-bg-tertiary hover:text-text-primary transition-colors">
            {t('alerts.undo')}
          </button>
        </div>
      )}
    </>
  );

  if (criticalAlerts.length === 0) {
    return triageError || lastDismissed.length > 0 ? notices : null;
  }

  return (
    <>
    {notices}
    <div className="mx-4 mt-2 mb-1 bg-amber-500/15 border border-amber-500/30 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-amber-400" />
          <span className="text-sm font-medium text-amber-400">
            {pendingAlerts.length === 1
              ? t('alerts.criticalSingular', 'Needs Attention')
              : t('alerts.criticalPlural', '{{count}} Items Need Attention', { count: pendingAlerts.length })}
          </span>
        </div>
        {pendingAlerts.length > 1 && (
          <button
            onClick={handleTriageAll}
            className="text-xs text-text-muted hover:text-text-secondary transition-colors"
          >
            {t('alerts.dismissAll', 'Dismiss all')}
          </button>
        )}
      </div>

      {/* Alert list */}
      <div className="px-3 pb-2 space-y-1.5">
        {criticalAlerts.map(alert => (
          <div
            key={alert.id}
            className="flex items-start justify-between gap-2 text-xs bg-amber-500/10 rounded px-2 py-1.5"
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
              {/* Evidence row */}
              <div className="flex items-center gap-1.5 mt-1 flex-wrap">
                {alert.advisory_id && (
                  <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-red-500/15 text-red-400">
                    {alert.advisory_id}
                  </span>
                )}
                {alert.cvss_score != null && (
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                    alert.cvss_score >= 9.0 ? 'bg-red-500/15 text-red-400'
                      : alert.cvss_score >= 7.0 ? 'bg-orange-500/15 text-orange-400'
                      : 'bg-yellow-500/15 text-yellow-400'
                  }`}>
                    CVSS {alert.cvss_score.toFixed(1)}
                  </span>
                )}
                {alert.matched_deps && alert.matched_deps.length > 0 && (
                  <span className="text-[10px] text-text-muted">
                    {alert.dependency_path === 'direct' ? '●' : '○'}{' '}
                    {alert.matched_deps[0]}
                    {alert.installed_version ? ` ${alert.installed_version}` : ''}
                    {alert.dependency_path ? ` (${alert.dependency_path})` : ''}
                  </span>
                )}
                {alert.fixed_version && (
                  <span className="text-[10px] text-emerald-400">
                    {'→'} {alert.fixed_version}
                  </span>
                )}
              </div>
            </div>
            <div className="flex items-center gap-1 shrink-0">
              {alert.url != null && alert.url !== '' && isSafeUrl(alert.url) && (
                <button
                  onClick={() => {
                    // Opening the advisory is not a decision about it.
                    import('@tauri-apps/plugin-opener').then(({ openUrl }) => {
                      void openUrl(alert.url!);
                    }).catch(() => {
                      window.open(alert.url, '_blank', 'noopener,noreferrer');
                    });
                  }}
                  className="px-1.5 py-0.5 text-[10px] rounded bg-blue-500/15 text-blue-400 hover:bg-blue-500/25 transition-colors"
                >
                  {t('alerts.investigate', 'Investigate')}
                </button>
              )}
              <button
                onClick={() => { handleTriage([alert], 'not_applicable'); }}
                className="px-1.5 py-0.5 text-[10px] rounded bg-zinc-500/15 text-text-muted hover:bg-zinc-500/25 transition-colors"
                title={t('alerts.notApplicable', 'Not applicable to this project')}
              >
                {t('alerts.notApplicableShort', 'N/A')}
              </button>
              <button
                onClick={() => { handleTriage([alert], 'fixed'); }}
                className="px-1.5 py-0.5 text-[10px] rounded bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25 transition-colors"
                title={t('alerts.alreadyFixed', 'Already fixed/updated')}
              >
                {'✓'}
              </button>
            </div>
          </div>
        ))}
        {pendingAlerts.length > SHOWN && (
          <p className="text-[10px] text-text-muted px-2">
            {t('alerts.moreNotShown', { count: pendingAlerts.length - SHOWN })}
          </p>
        )}
      </div>
    </div>
    </>
  );
}

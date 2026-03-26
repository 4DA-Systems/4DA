import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

const ACTION_CATEGORIES = [
  'license', 'member', 'signal', 'decision',
  'settings', 'source', 'export', 'admin', 'auth', 'webhook',
] as const;

const CATEGORY_COLORS: Record<string, string> = {
  member: 'bg-blue-500/15 text-blue-400 border-blue-500/30',
  signal: 'bg-accent-gold/15 text-accent-gold border-accent-gold/30',
  decision: 'bg-purple-500/15 text-purple-400 border-purple-500/30',
  settings: 'bg-gray-500/15 text-gray-400 border-gray-500/30',
  admin: 'bg-red-500/15 text-red-400 border-red-500/30',
  license: 'bg-green-500/15 text-green-400 border-green-500/30',
  auth: 'bg-orange-500/15 text-orange-400 border-orange-500/30',
  source: 'bg-cyan-500/15 text-cyan-400 border-cyan-500/30',
  export: 'bg-indigo-500/15 text-indigo-400 border-indigo-500/30',
  webhook: 'bg-pink-500/15 text-pink-400 border-pink-500/30',
};

const AVATAR_COLORS = [
  'bg-blue-500', 'bg-green-500', 'bg-purple-500', 'bg-orange-500',
  'bg-pink-500', 'bg-cyan-500', 'bg-yellow-500', 'bg-red-500',
];

function avatarColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
  return AVATAR_COLORS[Math.abs(hash) % AVATAR_COLORS.length]!;
}

function actionCategory(action: string): string {
  const dot = action.indexOf('.');
  return dot > 0 ? action.slice(0, dot) : action;
}

function formatRelativeTime(iso: string): string {
  const diff = Math.max(0, Date.now() - new Date(iso).getTime());
  const s = Math.floor(diff / 1000);
  if (s < 60) return 'just now';
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d}d ago`;
  return new Date(iso).toLocaleDateString();
}

function dateIso(daysAgo = 0): string {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  return d.toISOString().slice(0, 10);
}

const PAGE_SIZE = 50;

export function AuditLogViewer() {
  const { t } = useTranslation();

  const auditEntries = useAppStore(s => s.auditEntries);
  const auditSummary = useAppStore(s => s.auditSummary);
  const auditLoading = useAppStore(s => s.auditLoading);
  const auditActionFilter = useAppStore(s => s.auditActionFilter);
  const auditResourceFilter = useAppStore(s => s.auditResourceFilter);
  const loadAuditLog = useAppStore(s => s.loadAuditLog);
  const loadAuditSummary = useAppStore(s => s.loadAuditSummary);
  const exportAuditCsv = useAppStore(s => s.exportAuditCsv);
  const setAuditActionFilter = useAppStore(s => s.setAuditActionFilter);
  const setAuditResourceFilter = useAppStore(s => s.setAuditResourceFilter);

  const [expandedRows, setExpandedRows] = useState<Set<string>>(new Set());
  const [showSummary, setShowSummary] = useState(false);
  const [exportFrom, setExportFrom] = useState(() => dateIso(30));
  const [exportTo, setExportTo] = useState(() => dateIso(0));
  const [offset, setOffset] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setOffset(0);
    loadAuditLog(auditActionFilter || undefined, auditResourceFilter || undefined, PAGE_SIZE, 0)
      .catch(e => setError(String(e)));
    loadAuditSummary(30).catch((e) => console.debug('[AuditLogViewer] summary:', e));
  }, [auditActionFilter, auditResourceFilter, loadAuditLog, loadAuditSummary]);

  const handleRefresh = useCallback(() => {
    setOffset(0);
    setError(null);
    loadAuditLog(auditActionFilter || undefined, auditResourceFilter || undefined, PAGE_SIZE, 0)
      .catch(e => setError(String(e)));
    loadAuditSummary(30).catch((e) => console.debug('[AuditLogViewer] summary refresh:', e));
  }, [auditActionFilter, auditResourceFilter, loadAuditLog, loadAuditSummary]);

  const handleLoadMore = useCallback(() => {
    const next = offset + PAGE_SIZE;
    setOffset(next);
    loadAuditLog(auditActionFilter || undefined, auditResourceFilter || undefined, PAGE_SIZE, next)
      .catch(e => setError(String(e)));
  }, [offset, auditActionFilter, auditResourceFilter, loadAuditLog]);

  const handleExport = useCallback(async () => {
    try {
      const csv = await exportAuditCsv(exportFrom, exportTo);
      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `audit-log-${exportFrom}-to-${exportTo}.csv`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setError(String(e));
    }
  }, [exportAuditCsv, exportFrom, exportTo]);

  const toggleRow = useCallback((eventId: string) => {
    setExpandedRows(prev => {
      const next = new Set(prev);
      if (next.has(eventId)) next.delete(eventId); else next.add(eventId);
      return next;
    });
  }, []);

  const resourceTypes = useMemo(() => {
    const set = new Set<string>();
    auditEntries.forEach(e => set.add(e.resource_type));
    return Array.from(set).sort();
  }, [auditEntries]);

  const mostActiveAction = auditSummary?.events_by_action?.[0];
  const lastEvent = auditEntries.length > 0 ? auditEntries[0] : null;
  const summaryMaxDay = auditSummary
    ? Math.max(...auditSummary.events_by_day.map(([, n]) => n), 1)
    : 1;

  const selectCls = 'px-2 py-1.5 bg-bg-secondary border border-border rounded text-xs text-text-secondary focus:border-success/50 focus:outline-none';
  const dateCls = 'px-2 py-1.5 bg-bg-secondary border border-border rounded text-xs text-text-secondary focus:border-success/50 focus:outline-none';

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border" role="region" aria-label={t('enterprise.audit.title', 'Audit Log')}>
      {/* Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium text-white">{t('enterprise.audit.title', 'Audit Log')}</h3>
          <span className="text-[10px] px-1.5 py-0.5 bg-success/15 text-success rounded font-medium">
            {t('enterprise.audit.badge', 'Enterprise')}
          </span>
        </div>
        <button
          onClick={handleRefresh}
          disabled={auditLoading}
          aria-label={t('enterprise.audit.refresh', 'Refresh audit log')}
          className="p-1.5 text-text-muted hover:text-white rounded transition-colors disabled:opacity-50"
        >
          <svg className={`w-4 h-4 ${auditLoading ? 'animate-spin' : ''}`} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
        </button>
      </div>

      {/* Summary stats */}
      {auditSummary && (
        <div className="flex items-center gap-4 mb-3 text-[11px] text-text-muted">
          <span>{t('enterprise.audit.totalEvents', 'Total events')}: <span className="text-white font-medium">{auditSummary.total_events}</span></span>
          {mostActiveAction && (
            <span>{t('enterprise.audit.topAction', 'Top action')}: <span className="text-white font-medium">{mostActiveAction[0]}</span></span>
          )}
          {lastEvent && (
            <span>{t('enterprise.audit.lastEvent', 'Last event')}: <span className="text-white font-medium">{formatRelativeTime(lastEvent.created_at)}</span></span>
          )}
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-2 mb-3">
        <select value={auditActionFilter} onChange={e => setAuditActionFilter(e.target.value)} aria-label={t('enterprise.audit.filterAction', 'Filter by action')} className={selectCls}>
          <option value="">{t('enterprise.audit.allActions', 'All actions')}</option>
          {ACTION_CATEGORIES.map(c => <option key={c} value={`${c}.*`}>{c}.*</option>)}
        </select>
        <select value={auditResourceFilter} onChange={e => setAuditResourceFilter(e.target.value)} aria-label={t('enterprise.audit.filterResource', 'Filter by resource')} className={selectCls}>
          <option value="">{t('enterprise.audit.allResources', 'All resources')}</option>
          {resourceTypes.map(rt => <option key={rt} value={rt}>{rt}</option>)}
        </select>
        <div className="flex items-center gap-1 ms-auto">
          <input type="date" value={exportFrom} onChange={e => setExportFrom(e.target.value)} aria-label={t('enterprise.audit.fromDate', 'From date')} className={dateCls} />
          <span className="text-[10px] text-text-muted">&ndash;</span>
          <input type="date" value={exportTo} onChange={e => setExportTo(e.target.value)} aria-label={t('enterprise.audit.toDate', 'To date')} className={dateCls} />
          <button onClick={handleExport} className="px-3 py-1.5 text-xs font-medium text-success border border-success/30 rounded hover:bg-success/10 transition-colors">
            {t('enterprise.audit.exportCsv', 'Export CSV')}
          </button>
        </div>
      </div>

      {error && (
        <div className="text-xs text-red-400 p-3 mb-3 bg-red-500/10 rounded border border-red-500/20" role="alert">{error}</div>
      )}

      {/* Audit table */}
      <div className="overflow-x-auto">
        <table className="w-full text-xs" role="table" aria-label={t('enterprise.audit.tableLabel', 'Audit log entries')}>
          <thead>
            <tr className="text-start text-text-muted border-b border-border">
              <th className="pb-2 pe-3 font-medium">{t('enterprise.audit.colTime', 'Time')}</th>
              <th className="pb-2 pe-3 font-medium">{t('enterprise.audit.colActor', 'Actor')}</th>
              <th className="pb-2 pe-3 font-medium">{t('enterprise.audit.colAction', 'Action')}</th>
              <th className="pb-2 pe-3 font-medium">{t('enterprise.audit.colResource', 'Resource')}</th>
              <th className="pb-2 font-medium">{t('enterprise.audit.colDetails', 'Details')}</th>
            </tr>
          </thead>
          <tbody>
            {auditEntries.map(entry => {
              const cat = actionCategory(entry.action);
              const colorClass = CATEGORY_COLORS[cat] || CATEGORY_COLORS.settings;
              const isExpanded = expandedRows.has(entry.event_id);
              const hasDetails = entry.details != null;
              return (
                <tr key={entry.event_id} className="border-b border-border/50 hover:bg-bg-secondary/50 transition-colors">
                  <td className="py-2 pe-3 whitespace-nowrap text-text-muted" title={entry.created_at}>
                    {formatRelativeTime(entry.created_at)}
                  </td>
                  <td className="py-2 pe-3">
                    <div className="flex items-center gap-1.5">
                      <span className={`w-5 h-5 rounded-full flex items-center justify-center text-[10px] font-semibold text-white ${avatarColor(entry.actor_display_name)}`} aria-hidden="true">
                        {entry.actor_display_name.charAt(0).toUpperCase()}
                      </span>
                      <span className="text-text-secondary truncate max-w-[100px]" title={entry.actor_display_name}>
                        {entry.actor_display_name}
                      </span>
                    </div>
                  </td>
                  <td className="py-2 pe-3">
                    <span className={`inline-block px-1.5 py-0.5 rounded border text-[10px] font-medium ${colorClass}`}>
                      {entry.action}
                    </span>
                  </td>
                  <td className="py-2 pe-3 text-text-muted">
                    <span>{entry.resource_type}</span>
                    {entry.resource_id && (
                      <span className="ms-1 text-text-muted/60" title={entry.resource_id}>
                        {entry.resource_id.length > 12 ? `${entry.resource_id.slice(0, 12)}...` : entry.resource_id}
                      </span>
                    )}
                  </td>
                  <td className="py-2">
                    {hasDetails ? (
                      <button onClick={() => toggleRow(entry.event_id)} aria-expanded={isExpanded} aria-label={t('enterprise.audit.toggleDetails', 'Toggle details')} className="text-text-muted hover:text-white transition-colors">
                        {isExpanded ? '\u25BE' : '\u25B8'}
                      </button>
                    ) : (
                      <span className="text-text-muted/40">&mdash;</span>
                    )}
                    {isExpanded && hasDetails && (
                      <pre className="mt-1 p-2 bg-bg-primary rounded text-[10px] text-text-secondary overflow-x-auto max-w-[300px]">
                        {JSON.stringify(entry.details, null, 2)}
                      </pre>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Empty state */}
      {!auditLoading && auditEntries.length === 0 && (
        <p className="text-xs text-text-muted text-center py-6">{t('enterprise.audit.empty', 'No audit events recorded yet')}</p>
      )}

      {/* Loading */}
      {auditLoading && (
        <div className="flex items-center justify-center gap-2 py-4">
          <div className="w-4 h-4 border-2 border-success border-t-transparent rounded-full animate-spin" />
          <span className="text-xs text-text-muted">{t('enterprise.audit.loading', 'Loading audit log...')}</span>
        </div>
      )}

      {/* Load more */}
      {!auditLoading && auditEntries.length > 0 && auditEntries.length >= offset + PAGE_SIZE && (
        <div className="flex justify-center mt-3">
          <button onClick={handleLoadMore} className="px-4 py-1.5 text-xs text-text-secondary border border-border rounded hover:text-white hover:border-success/30 transition-colors">
            {t('enterprise.audit.loadMore', 'Load more')}
          </button>
        </div>
      )}

      {/* Summary panel */}
      {auditSummary && (
        <div className="mt-4 border-t border-border pt-3">
          <button onClick={() => setShowSummary(!showSummary)} aria-expanded={showSummary} className="flex items-center gap-1 text-xs text-text-muted hover:text-white transition-colors mb-2">
            <span>{showSummary ? '\u25BE' : '\u25B8'}</span>
            <span className="font-medium">{t('enterprise.audit.summary', 'Summary')}</span>
          </button>
          {showSummary && (
            <div className="space-y-4">
              {auditSummary.events_by_day.length > 0 && (
                <div>
                  <h4 className="text-[11px] text-text-secondary font-medium mb-1.5">{t('enterprise.audit.eventsByDay', 'Events by day')}</h4>
                  <div className="space-y-1">
                    {auditSummary.events_by_day.slice(-14).map(([day, count]) => (
                      <div key={day} className="flex items-center gap-2">
                        <span className="text-[10px] text-text-muted w-16 shrink-0">{day.slice(5)}</span>
                        <div className="flex-1 h-2.5 bg-bg-secondary rounded-full overflow-hidden">
                          <div className="h-full bg-success/50 rounded-full transition-all" style={{ width: `${(count / summaryMaxDay) * 100}%` }} />
                        </div>
                        <span className="text-[10px] text-text-muted w-6 text-end">{count}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {auditSummary.events_by_actor.length > 0 && (
                <div>
                  <h4 className="text-[11px] text-text-secondary font-medium mb-1.5">{t('enterprise.audit.topActors', 'Top actors')}</h4>
                  <div className="flex flex-wrap gap-1.5">
                    {auditSummary.events_by_actor.slice(0, 8).map(([actor, count]) => (
                      <span key={actor} className="px-2 py-1 text-[10px] bg-bg-secondary text-text-secondary rounded border border-border">{actor} ({count})</span>
                    ))}
                  </div>
                </div>
              )}
              {auditSummary.events_by_action.length > 0 && (
                <div>
                  <h4 className="text-[11px] text-text-secondary font-medium mb-1.5">{t('enterprise.audit.topActions', 'Top actions')}</h4>
                  <div className="flex flex-wrap gap-1.5">
                    {auditSummary.events_by_action.slice(0, 8).map(([action, count]) => (
                      <span key={action} className={`px-2 py-1 text-[10px] rounded border ${CATEGORY_COLORS[actionCategory(action)] || CATEGORY_COLORS.settings}`}>
                        {action} ({count})
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

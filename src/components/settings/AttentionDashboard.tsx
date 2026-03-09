import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { AttentionReport } from '../../types';

export function AttentionDashboard() {
  const { t } = useTranslation();
  const [report, setReport] = useState<AttentionReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [period, setPeriod] = useState(7);
  const cache = useRef<Record<number, AttentionReport>>({});

  const loadReport = async (days: number) => {
    if (cache.current[days]) {
      setReport(cache.current[days]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const r = await cmd('get_attention_report', { periodDays: days });
      cache.current[days] = r;
      setReport(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const timer = setTimeout(() => {
      loadReport(period);
    }, 300);
    return () => clearTimeout(timer);
  }, [period]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-indigo-500/20 rounded-lg flex items-center justify-center">
            <span>👁</span>
          </div>
          <div>
            <h3 className="text-sm font-medium text-white">{t('settings.attention.title')}</h3>
            <p className="text-xs text-text-muted">{t('settings.attention.description')}</p>
          </div>
        </div>
        <select
          value={period}
          onChange={(e) => setPeriod(Number(e.target.value))}
          className="bg-bg-tertiary border border-border rounded px-2 py-1 text-xs text-text-secondary"
        >
          <option value={7}>{t('settings.attention.days', { count: 7 })}</option>
          <option value={14}>{t('settings.attention.days', { count: 14 })}</option>
          <option value={30}>{t('settings.attention.days', { count: 30 })}</option>
        </select>
      </div>

      {loading && (
        <div className="flex items-center gap-2 py-4 justify-center">
          <div className="w-4 h-4 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs text-text-muted">{t('settings.attention.loading')}</span>
        </div>
      )}

      {error && (
        <div className="text-xs text-red-400 p-3 bg-red-500/10 rounded border border-red-500/20">{error}</div>
      )}

      {report && !loading && (
        <>
          {/* Topic Engagement */}
          {report.topic_engagement.length > 0 && (
            <div>
              <h4 className="text-xs font-medium text-text-secondary mb-2">{t('settings.attention.engagement')}</h4>
              <div className="space-y-1.5">
                {report.topic_engagement.slice(0, 8).map((te) => (
                  <div key={te.topic} className="flex items-center gap-2">
                    <span className="text-[11px] text-text-secondary w-24 truncate" title={te.topic}>{te.topic}</span>
                    <div className="flex-1 h-3 bg-bg-tertiary rounded-full overflow-hidden">
                      <div
                        className="h-full bg-indigo-500/60 rounded-full transition-all"
                        style={{ width: `${Math.min(te.percent_of_total, 100)}%` }}
                      />
                    </div>
                    <span className="text-[10px] text-text-muted w-10 text-right">{Math.round(te.percent_of_total)}%</span>
                    <span className="text-[10px] text-text-muted w-6 text-right">{te.interactions}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Codebase Topics */}
          {report.codebase_topics.length > 0 && (
            <div>
              <h4 className="text-xs font-medium text-text-secondary mb-2">{t('settings.attention.codebaseTopics')}</h4>
              <div className="flex flex-wrap gap-1.5">
                {report.codebase_topics.slice(0, 12).map((ct) => (
                  <span key={ct.topic} className="px-2 py-1 text-[10px] bg-emerald-500/10 text-emerald-400 rounded border border-emerald-500/20">
                    {ct.topic} ({ct.file_count})
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Blind Spots */}
          {report.blind_spots.length > 0 && (
            <div>
              <h4 className="text-xs font-medium text-text-secondary mb-2">{t('settings.attention.blindSpots')}</h4>
              <div className="space-y-2">
                {report.blind_spots.map((bs) => (
                  <div key={bs.topic} className="px-3 py-2 bg-amber-500/5 border border-amber-500/20 rounded">
                    <div className="flex items-center gap-2">
                      <span className={`w-2 h-2 rounded-full ${
                        bs.risk_level === 'high' ? 'bg-red-400' :
                        bs.risk_level === 'medium' ? 'bg-amber-400' : 'bg-gray-400'
                      }`} />
                      <span className="text-xs text-amber-300 font-medium">{bs.topic}</span>
                      <span className="text-[10px] text-text-muted ml-auto">{t('settings.attention.risk', { level: bs.risk_level })}</span>
                    </div>
                    <p className="text-[11px] text-text-secondary mt-1">{bs.gap_description}</p>
                  </div>
                ))}
              </div>
            </div>
          )}

          {report.topic_engagement.length === 0 && report.blind_spots.length === 0 && (
            <p className="text-xs text-text-muted text-center py-4">
              {t('settings.attention.noData')}
            </p>
          )}
        </>
      )}
    </div>
  );
}

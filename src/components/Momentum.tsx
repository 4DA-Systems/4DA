import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';

import { MomentumPulse } from './momentum/MomentumPulse';
import { MomentumNarratives } from './momentum/MomentumNarratives';
import { MomentumDecisions } from './momentum/MomentumDecisions';
import { RadarEntryPanel } from './tech-radar/RadarEntryPanel';
import type { RadarEntry } from './tech-radar/RadarSVG';

interface TechRadarData {
  generated_at: string;
  entries: RadarEntry[];
}

export const Momentum = memo(function Momentum() {
  const { t } = useTranslation();
  const [data, setData] = useState<TechRadarData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<RadarEntry | null>(null);
  const [userStack, setUserStack] = useState<string[]>([]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    void Promise.allSettled([
      cmd('get_tech_radar'),
      cmd('get_user_context'),
    ]).then(([radarResult, ctxResult]) => {
      if (radarResult.status === 'fulfilled') {
        setData(radarResult.value as unknown as TechRadarData);
      } else {
        setError(String(radarResult.reason));
      }
      if (ctxResult.status === 'fulfilled') {
        setUserStack((ctxResult.value as { tech_stack: string[] }).tech_stack);
      }
    }).finally(() => setLoading(false));
  }, []);

  const handleEntryClick = useCallback((entry: RadarEntry) => setSelectedEntry(entry), []);
  const handleClosePanel = useCallback(() => setSelectedEntry(null), []);

  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 flex flex-col items-center justify-center gap-2">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin" />
        <div className="text-xs text-text-muted">{t('momentum.loading')}</div>
      </div>
    );
  }

  if (error !== null) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <p className="text-text-secondary text-sm">{t('error.generic')}</p>
      </div>
    );
  }

  if (!data || data.entries.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <p className="text-sm text-text-muted">{t('momentum.empty')}</p>
        <p className="text-xs text-text-muted mt-1">{t('momentum.emptyHint')}</p>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center gap-3">
        <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-text-secondary">
            <path d="M2 12L5 5L8 8L11 3L14 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            <circle cx="5" cy="5" r="1.5" fill="currentColor" opacity="0.4" />
            <circle cx="11" cy="3" r="1.5" fill="currentColor" opacity="0.4" />
          </svg>
        </div>
        <div className="flex-1">
          <h2 className="font-medium text-white text-sm">{t('momentum.title')}</h2>
          <p className="text-xs text-text-muted">{t('momentum.subtitle')}</p>
        </div>
        {data.generated_at && (
          <span className="text-[10px] text-text-muted">
            {new Date(data.generated_at).toLocaleDateString()}
          </span>
        )}
      </div>

      {/* The Pulse */}
      <MomentumPulse entries={data.entries} userStack={userStack} onEntryClick={handleEntryClick} />

      {/* What's Happening */}
      <MomentumNarratives entries={data.entries} userStack={userStack} onEntryClick={handleEntryClick} />

      {/* Decision Moments */}
      <MomentumDecisions entries={data.entries} userStack={userStack} />

      {/* Entry Detail Panel */}
      <RadarEntryPanel entry={selectedEntry} onClose={handleClosePanel} />
    </div>
  );
});

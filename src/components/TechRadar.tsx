import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { formatLocalDate } from '../utils/format-date';

import { StackIntelligence } from './tech-radar/StackIntelligence';
import { RadarEntryPanel } from './tech-radar/RadarEntryPanel';
import { TemporalSlider } from './tech-radar/TemporalSlider';
import type { RadarEntry } from './tech-radar/RadarSVG';

interface TechRadarData {
  generated_at: string;
  entries: RadarEntry[];
}

export const TechRadar = memo(function TechRadar() {
  const { t } = useTranslation();
  const [data, setData] = useState<TechRadarData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<RadarEntry | null>(null);
  const [userStack, setUserStack] = useState<string[]>([]);

  const loadRadar = useCallback(() => {
    setLoading(true);
    setError(null);
    cmd('get_tech_radar')
      .then(r => r as unknown as TechRadarData)
      .then(setData)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    loadRadar();
  }, [loadRadar]);

  useEffect(() => {
    cmd('get_user_context')
      .then((ctx) => setUserStack(ctx.tech_stack))
      .catch((e) => console.warn('TechRadar: failed to load context', e));
  }, []);

  const handleEntryClick = useCallback((entry: RadarEntry) => {
    setSelectedEntry(entry);
  }, []);

  const handleClosePanel = useCallback(() => {
    setSelectedEntry(null);
  }, []);

  const handleSnapshotChange = useCallback((date: string | null) => {
    if (date !== null) {
      cmd('get_radar_at_snapshot', { snapshotDate: date })
        .then((snapshot) => {
          const typed = snapshot as unknown as { entries?: unknown[] } | null;
          if (typed != null && Array.isArray(typed.entries)) {
            setData(snapshot as unknown as TechRadarData);
          }
        })
        .catch((e: unknown) => console.warn('TechRadar: failed to load snapshot', e));
    } else {
      cmd('get_tech_radar')
        .then(r => r as unknown as TechRadarData)
        .then(setData)
        .catch((e: unknown) => console.warn('TechRadar: failed to reload', e));
    }
  }, []);

  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 flex flex-col items-center justify-center gap-2">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin" aria-hidden="true" />
        <div className="text-xs text-text-muted" role="status">{t('techRadar.loading')}</div>
      </div>
    );
  }

  if (error !== null) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8">
        <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
          <p className="text-text-secondary text-sm">{t('error.generic')}</p>
          <button
            onClick={loadRadar}
            aria-label={t('action.retry')}
            className="px-3 py-1.5 text-xs bg-bg-tertiary hover:bg-white/10 rounded transition-colors text-text-secondary"
          >
            {t('action.retry')}
          </button>
        </div>
      </div>
    );
  }

  if (!data || data.entries.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <div className="text-sm text-text-muted">{t('techRadar.empty')}</div>
        <div className="text-xs text-text-muted mt-1">
          {t('techRadar.emptyHint')}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center gap-3">
        <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-text-secondary" aria-hidden="true">
            <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="1.2" />
            <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1" opacity="0.4" />
            <line x1="8" y1="1" x2="8" y2="4" stroke="currentColor" strokeWidth="1" opacity="0.3" />
            <line x1="8" y1="12" x2="8" y2="15" stroke="currentColor" strokeWidth="1" opacity="0.3" />
            <line x1="1" y1="8" x2="4" y2="8" stroke="currentColor" strokeWidth="1" opacity="0.3" />
            <line x1="12" y1="8" x2="15" y2="8" stroke="currentColor" strokeWidth="1" opacity="0.3" />
          </svg>
        </div>
        <div className="flex-1">
          <h2 className="font-medium text-white text-sm">
            {t('techRadar.title', 'Stack Intelligence')}
          </h2>
          <p className="text-xs text-text-muted">
            {t('techRadar.subtitle', 'Your technology landscape')}
          </p>
        </div>
        {data.generated_at && (
          <span className="text-[10px] text-text-muted">
            {formatLocalDate(data.generated_at)}
          </span>
        )}
      </div>

      {/* Stack Intelligence */}
      <div className="flex justify-center">
        <StackIntelligence
          entries={data.entries}
          userStack={userStack}
          onEntryClick={handleEntryClick}
        />
      </div>

      {/* Temporal Slider */}
      <TemporalSlider onSnapshotChange={handleSnapshotChange} />

      {/* Legend */}
      <div className="px-5 py-2.5 border-t border-border flex items-center gap-5 text-[10px] text-text-muted" role="group" aria-label={t('techRadar.legend', 'Legend')}>
        <div className="flex items-center gap-1.5">
          <span className="text-green-400" aria-hidden="true">{'\u2191'}</span>
          <span>{t('techRadar.movingIn', 'Rising')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="text-red-400" aria-hidden="true">{'\u2193'}</span>
          <span>{t('techRadar.movingOut', 'Declining')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="text-accent-gold" aria-hidden="true">{'\u2726'}</span>
          <span>{t('techRadar.new', 'New')}</span>
        </div>
      </div>

      {/* Entry Detail Panel */}
      <RadarEntryPanel entry={selectedEntry} onClose={handleClosePanel} />
    </div>
  );
});

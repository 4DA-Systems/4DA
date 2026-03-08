import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { useGameComponent } from '../hooks/use-game-component';

import { RadarSVG } from './tech-radar/RadarSVG';
import { RadarEntryPanel } from './tech-radar/RadarEntryPanel';
import { TemporalSlider } from './tech-radar/TemporalSlider';
import type { RadarEntry } from './tech-radar/RadarSVG';

interface TechRadarData {
  generated_at: string;
  entries: RadarEntry[];
}

function RadarField({ entries, userStack }: { entries: RadarEntry[]; userStack: string[] }) {
  const { containerRef, elementRef } = useGameComponent('game-radar-field');

  useEffect(() => {
    const el = elementRef.current;
    if (!el || entries.length === 0) return;
    const total = entries.length;
    const byQuad = (q: string) => entries.filter(e => e.quadrant === q).length / total;
    el.setParam?.('lang_energy', byQuad('languages'));
    el.setParam?.('fw_energy', byQuad('frameworks'));
    el.setParam?.('tool_energy', byQuad('tools'));
    el.setParam?.('plat_energy', byQuad('platforms'));
    el.setParam?.('moving_in', entries.filter(e => e.movement === 'up').length / total);
    el.setParam?.('moving_out', entries.filter(e => e.movement === 'down').length / total);
    const stackLower = userStack.map(s => s.toLowerCase());
    el.setParam?.('stack_glow', entries.filter(e => stackLower.includes(e.name.toLowerCase())).length / total);
  }, [entries, userStack, elementRef]);

  return <div ref={containerRef} className="absolute inset-0 rounded-lg overflow-hidden" aria-hidden="true" />;
}

export const TechRadar = memo(function TechRadar() {
  const { t } = useTranslation();
  const [data, setData] = useState<TechRadarData | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedEntry, setSelectedEntry] = useState<RadarEntry | null>(null);
  const [userStack, setUserStack] = useState<string[]>([]);

  // Load radar data
  useEffect(() => {
    invoke<TechRadarData>('get_tech_radar')
      .then(setData)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  // Load user's tech stack for highlighting
  useEffect(() => {
    invoke<{ tech_stack: string[] }>('get_user_context')
      .then((ctx) => setUserStack(ctx.tech_stack))
      .catch(() => {});
  }, []);

  const handleEntryClick = useCallback((entry: RadarEntry) => {
    setSelectedEntry(entry);
  }, []);

  const handleClosePanel = useCallback(() => {
    setSelectedEntry(null);
  }, []);

  const handleSnapshotChange = useCallback((date: string | null) => {
    if (date) {
      invoke<TechRadarData>('get_radar_at_snapshot', { snapshotDate: date })
        .then((snapshot) => {
          if (snapshot && (snapshot as unknown as { entries?: unknown[] }).entries) {
            setData(snapshot as TechRadarData);
          }
        })
        .catch(() => {});
    } else {
      // Reload current radar
      invoke<TechRadarData>('get_tech_radar').then(setData).catch(() => {});
    }
  }, []);

  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 flex flex-col items-center justify-center gap-2">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin" />
        <div className="text-xs text-gray-500">{t('techRadar.loading')}</div>
      </div>
    );
  }

  if (!data || data.entries.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <div className="text-sm text-gray-500">{t('techRadar.empty')}</div>
        <div className="text-xs text-gray-600 mt-1">
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
          <span className="text-sm text-gray-400">R</span>
        </div>
        <div>
          <h2 className="font-medium text-white text-sm">{t('techRadar.title')}</h2>
          <p className="text-xs text-gray-500">
            {t('techRadar.count', { count: data.entries.length })}
          </p>
        </div>
      </div>

      {/* Radar SVG with GAME background */}
      <div className="relative p-4 flex justify-center">
        <RadarField entries={data.entries} userStack={userStack} />
        <RadarSVG
          entries={data.entries}
          userStack={userStack}
          onEntryClick={handleEntryClick}
        />
      </div>

      {/* Temporal Slider */}
      <TemporalSlider onSnapshotChange={handleSnapshotChange} />

      {/* Legend */}
      <div className="px-5 py-3 border-t border-border flex items-center gap-5 text-[10px] text-gray-500">
        <div className="flex items-center gap-1.5">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <polygon points="5,1 2,7 8,7" fill="#22C55E" />
          </svg>
          <span>{t('techRadar.movingIn')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <polygon points="5,9 2,3 8,3" fill="#EF4444" />
          </svg>
          <span>{t('techRadar.movingOut')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <polygon points="5,1 9,5 5,9 1,5" fill="#D4AF37" />
          </svg>
          <span>{t('techRadar.new')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="8" height="8" viewBox="0 0 8 8">
            <circle cx="4" cy="4" r="3" fill="#FFFFFF" />
          </svg>
          <span>{t('techRadar.stable')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="12" height="12" viewBox="0 0 12 12">
            <circle cx="6" cy="6" r="3" fill="#FFFFFF" />
            <circle cx="6" cy="6" r="5" fill="none" stroke="#D4AF37" strokeWidth="1.5" />
          </svg>
          <span>{t('techRadar.yourStack')}</span>
        </div>
        {data.generated_at && (
          <span className="ml-auto text-gray-600">
            {t('techRadar.generated', { date: new Date(data.generated_at).toLocaleDateString() })}
          </span>
        )}
      </div>

      {/* Entry Detail Panel */}
      <RadarEntryPanel entry={selectedEntry} onClose={handleClosePanel} />
    </div>
  );
});

// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, useCallback, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { cmd } from '../lib/commands';

import { AttentionSection } from './momentum/AttentionSection';
import { MovingSection } from './momentum/MovingSection';
import { StackGlance } from './momentum/StackGlance';
import { RadarEntryPanel } from './tech-radar/RadarEntryPanel';
import { buildAttentionItems } from './momentum/momentum-utils';

import type { RadarEntry } from './tech-radar/RadarSVG';
import type { KnowledgeGap } from '../types/innovation';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface TechRadarData {
  generated_at: string;
  entries: RadarEntry[];
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const Momentum = memo(function Momentum() {
  const { t } = useTranslation();

  // Radar + context data
  const [radarData, setRadarData] = useState<TechRadarData | null>(null);
  const [userStack, setUserStack] = useState<string[]>([]);
  const [gaps, setGaps] = useState<KnowledgeGap[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Detail panel
  const [selectedEntry, setSelectedEntry] = useState<RadarEntry | null>(null);

  // Decision windows from store
  const windows = useAppStore(s => s.decisionWindows);
  const loadWindows = useAppStore(s => s.loadDecisionWindows);
  const actOnWindow = useAppStore(s => s.actOnWindow);
  const closeWindow = useAppStore(s => s.closeWindow);

  // Load all data on mount
  useEffect(() => {
    setLoading(true);
    setError(null);
    void Promise.allSettled([
      cmd('get_tech_radar'),
      cmd('get_user_context'),
      cmd('get_knowledge_gaps'),
      loadWindows(),
    ]).then(([radarResult, ctxResult, gapsResult]) => {
      if (radarResult.status === 'fulfilled') {
        setRadarData(radarResult.value as unknown as TechRadarData);
      } else {
        setError(String(radarResult.reason));
      }
      if (ctxResult.status === 'fulfilled') {
        setUserStack((ctxResult.value as { tech_stack: string[] }).tech_stack);
      }
      if (gapsResult.status === 'fulfilled') {
        setGaps(gapsResult.value as KnowledgeGap[]);
      }
    }).finally(() => setLoading(false));
  }, [loadWindows]);

  // Handlers
  const handleEntryClick = useCallback((entry: RadarEntry) => setSelectedEntry(entry), []);
  const handleClosePanel = useCallback(() => setSelectedEntry(null), []);

  const handleViewEntry = useCallback((name: string) => {
    const entry = radarData?.entries.find(e => e.name === name);
    if (entry) setSelectedEntry(entry);
  }, [radarData]);

  // Build attention items
  const attentionItems = useMemo(
    () => buildAttentionItems(radarData?.entries ?? [], userStack, windows, gaps, t),
    [radarData, userStack, windows, gaps, t],
  );

  // Loading state
  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 flex flex-col items-center justify-center gap-2">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin" />
        <div className="text-xs text-text-muted">{t('momentum.loading')}</div>
      </div>
    );
  }

  // Error state
  if (error !== null) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <p className="text-text-secondary text-sm">{t('error.generic')}</p>
      </div>
    );
  }

  // Empty state
  if (!radarData || radarData.entries.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <p className="text-sm text-text-muted">{t('momentum.empty')}</p>
        <p className="text-xs text-text-muted mt-1">{t('momentum.emptyHint')}</p>
      </div>
    );
  }

  const entries = radarData.entries;

  return (
    <>
      <div className="space-y-6">
        {/* Page header */}
        <div className="flex items-center gap-3">
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
          {radarData.generated_at && (
            <span className="text-[10px] text-text-muted">
              {new Date(radarData.generated_at).toLocaleDateString()}
            </span>
          )}
        </div>

        {/* Section 1: What Needs Attention */}
        <AttentionSection
          items={attentionItems}
          techCount={entries.length}
          onViewEntry={handleViewEntry}
          onActOnWindow={actOnWindow}
          onCloseWindow={closeWindow}
        />

        {/* Section 2: What's Moving */}
        <MovingSection
          entries={entries}
          userStack={userStack}
          onEntryClick={handleEntryClick}
        />

        {/* Section 3: Your Stack at a Glance */}
        <StackGlance
          entries={entries}
          userStack={userStack}
          onEntryClick={handleEntryClick}
        />
      </div>

      {/* Entry Detail Panel (overlay) */}
      <RadarEntryPanel entry={selectedEntry} onClose={handleClosePanel} />
    </>
  );
});

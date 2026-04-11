// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, useCallback, useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { cmd } from '../lib/commands';
import { MomentumHero } from './momentum/MomentumHero';
import { ActiveWorkSection, type ActiveWorkData } from './momentum/ActiveWorkSection';
import { AttentionSection } from './momentum/AttentionSection';
import { MovingSection } from './momentum/MovingSection';
import { PositioningSection } from './momentum/PositioningSection';
import { StackGlance } from './momentum/StackGlance';
import { MomentumWisdomTrajectory } from './awe/MomentumWisdomTrajectory';
import { RadarEntryPanel } from './tech-radar/RadarEntryPanel';
import { buildAttentionItems } from './momentum/momentum-utils';

import type { RadarEntry } from './tech-radar/RadarSVG';
import type { KnowledgeGap, SignalChainWithPrediction } from '../types/innovation';
import type { CompoundAdvantageScore as CompoundScore } from '../types/autophagy';

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

  // Core data
  const [radarData, setRadarData] = useState<TechRadarData | null>(null);
  const [userStack, setUserStack] = useState<string[]>([]);
  const [gaps, setGaps] = useState<KnowledgeGap[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Positioning data
  const [advantage, setAdvantage] = useState<CompoundScore | null>(null);
  const [history, setHistory] = useState<number[]>([]);
  const [chains, setChains] = useState<SignalChainWithPrediction[]>([]);
  const [aweData, setAweData] = useState<{ principles?: string[] } | null>(null);

  // Active work context
  const [activeWork, setActiveWork] = useState<ActiveWorkData | null>(null);

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
      cmd('get_compound_advantage'),
      cmd('get_advantage_history', { period: 'weekly', limit: 8 }),
      cmd('get_signal_chains_predicted'),
      cmd('run_awe_recall', { domain: 'software-engineering' }),
      cmd('get_active_work_context'),
    ]).then(([radarR, ctxR, gapsR, , advR, histR, chainsR, aweR, workR]) => {
      if (radarR.status === 'fulfilled') setRadarData(radarR.value as unknown as TechRadarData);
      else setError(String(radarR.reason));
      if (ctxR.status === 'fulfilled') setUserStack((ctxR.value as { tech_stack: string[] }).tech_stack);
      if (gapsR.status === 'fulfilled') setGaps(gapsR.value as KnowledgeGap[]);
      if (advR.status === 'fulfilled') setAdvantage(advR.value as CompoundScore);
      if (histR.status === 'fulfilled') setHistory(histR.value as number[]);
      if (chainsR.status === 'fulfilled') setChains(chainsR.value as SignalChainWithPrediction[]);
      if (aweR.status === 'fulfilled') {
        // run_awe_recall returns the PLAIN TEXT output of `awe wisdom -d <domain>`.
        // The previous code tried JSON.parse and silently fell back to null for every
        // user (the real AWE CLI never emitted JSON for this command), which is why
        // the flagship AWE panel has looked static for a week. Parse the text format:
        // lines like "[85%] Some principle statement" under a "VALIDATED PRINCIPLES"
        // or "CANDIDATE PRINCIPLES" section header.
        try {
          const raw = typeof aweR.value === 'string' ? aweR.value : String(aweR.value);
          const principles: string[] = [];
          let inPrincipleSection = false;
          for (const line of raw.split(/\r?\n/)) {
            const trimmed = line.trim();
            if (trimmed.includes('VALIDATED PRINCIPLES') || trimmed.includes('CANDIDATE PRINCIPLES')) {
              inPrincipleSection = true;
              continue;
            }
            if (trimmed.includes('ANTI-PATTERNS') || trimmed.startsWith('---')) {
              inPrincipleSection = false;
              continue;
            }
            if (inPrincipleSection && /^\[\s*\d+\s*%?\s*\]/.test(trimmed)) {
              const afterBracket = trimmed.split(']').slice(1).join(']').trim();
              if (afterBracket.length > 0
                  && !afterBracket.startsWith('Evidence')
                  && !afterBracket.startsWith('Status')) {
                principles.push(afterBracket);
              }
            }
          }
          setAweData({ principles });
        } catch { setAweData(null); }
      }
      if (workR.status === 'fulfilled') {
        setActiveWork(workR.value as ActiveWorkData);
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

  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 flex flex-col items-center justify-center gap-2">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin" aria-hidden="true" />
        <div className="text-xs text-text-muted" role="status">{t('momentum.loading')}</div>
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
      <div className="space-y-6" role="region" aria-label={t('momentum.title', 'Momentum')}>
        {/* Hero: Precision gauge row */}
        <MomentumHero advantage={advantage} history={history} entries={entries} gaps={gaps} />

        {/* Active Work Context */}
        <ActiveWorkSection data={activeWork} />

        {/* What Needs Attention */}
        <AttentionSection
          items={attentionItems}
          techCount={entries.length}
          onViewEntry={handleViewEntry}
          onActOnWindow={(id: number) => { void actOnWindow(id); }}
          onCloseWindow={(id: number) => { void closeWindow(id); }}
        />

        {/* What's Moving */}
        <MovingSection entries={entries} userStack={userStack} onEntryClick={handleEntryClick} />

        {/* Positioning: Signal Chains + AWE Wisdom */}
        <PositioningSection chains={chains} aweData={aweData} advantage={advantage} />

        {/* AWE Wisdom Trajectory */}
        <MomentumWisdomTrajectory />

        {/* Your Stack */}
        <StackGlance entries={entries} userStack={userStack} onEntryClick={handleEntryClick} />
      </div>

      <RadarEntryPanel entry={selectedEntry} onClose={handleClosePanel} />
    </>
  );
});

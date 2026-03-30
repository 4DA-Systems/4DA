// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { AccuracyTab } from './intelligence/AccuracyTab';
import { ConvergenceTab } from './intelligence/ConvergenceTab';
import { CostTab } from './intelligence/CostTab';
import { WisdomTab } from './intelligence/WisdomTab';

// ============================================================================
// Types
// ============================================================================

type ConsoleTab = 'accuracy' | 'convergence' | 'costs' | 'wisdom';

const TAB_IDS: ConsoleTab[] = ['accuracy', 'convergence', 'costs', 'wisdom'];

// ============================================================================
// TabButton
// ============================================================================

function TabButton({
  id,
  active,
  label,
  description,
  controls,
  onClick,
}: {
  id: string;
  active: boolean;
  label: string;
  description: string;
  controls: string;
  onClick: () => void;
}) {
  return (
    <button
      id={id}
      role="tab"
      aria-selected={active}
      aria-controls={controls}
      onClick={onClick}
      className={`px-5 py-3 text-start transition-colors border-b-2 ${
        active
          ? 'border-accent-gold text-white'
          : 'border-transparent text-text-muted hover:text-text-secondary hover:border-border'
      }`}
    >
      <span className="text-sm font-medium block">{label}</span>
      <span className={`text-[10px] block mt-0.5 ${active ? 'text-text-secondary' : 'text-text-muted/60'}`}>
        {description}
      </span>
    </button>
  );
}

// ============================================================================
// IntelligenceConsole
// ============================================================================

export const IntelligenceConsole = memo(function IntelligenceConsole() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<ConsoleTab>('accuracy');
  const [hasAnyData, setHasAnyData] = useState<boolean | null>(null);

  const TABS = TAB_IDS.map(id => ({
    id,
    label: t(`console.tab_${id}` as const),
    description: t(`console.tabDesc_${id}` as const),
  }));

  useEffect(() => {
    Promise.allSettled([
      cmd('get_accuracy_report', { period: 'month' }),
      cmd('get_tech_convergence'),
      cmd('get_ai_usage_summary', { period: 'month' }),
    ]).then(results => {
      const anyData = results.some(r => {
        if (r.status !== 'fulfilled' || !r.value) return false;
        const v = r.value;
        if (Array.isArray(v)) return v.length > 0;
        if (typeof v === 'object') {
          // Check for meaningful data beyond empty shell responses
          const keys = Object.keys(v);
          if (keys.length === 0) return false;
          // AiUsageSummary: check total_cost_usd or by_provider
          if ('total_cost_usd' in v && (v as unknown as Record<string, unknown>).total_cost_usd === 0) return false;
          return true;
        }
        return false;
      });
      setHasAnyData(anyData);
    });
  }, []);

  // Show a compelling "growing" state when all tabs would be empty
  if (hasAnyData === false) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden flex flex-col">
        <div className="px-5 py-4 border-b border-border">
          <h3 className="text-sm font-medium text-white">{t('console.title')}</h3>
          <p className="text-xs text-text-muted mt-1">
            {t('console.subtitle')}
          </p>
        </div>
        <div className="flex flex-col items-center justify-center h-64 text-center px-8">
          <div className="w-3 h-3 rounded-full bg-success/60 animate-pulse mb-4" />
          <h3 className="text-sm font-medium text-text-primary mb-2">{t('console.growing')}</h3>
          <p className="text-xs text-text-muted max-w-xs">
            {t('console.learningPatterns')}
          </p>
          <p className="text-xs text-text-muted/60 mt-3">
            {t('console.firstInsights')}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden flex flex-col">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">{t('console.title')}</h3>
        <p className="text-xs text-text-muted mt-1">
          {t('console.subtitle')}
        </p>
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-border" role="tablist" aria-label="Intelligence console tabs">
        {TABS.map(tabDef => (
          <TabButton
            key={tabDef.id}
            id={`tab-${tabDef.id}`}
            active={tab === tabDef.id}
            label={tabDef.label}
            description={tabDef.description}
            controls={`tabpanel-${tabDef.id}`}
            onClick={() => setTab(tabDef.id)}
          />
        ))}
      </div>

      {/* Tab content */}
      <div id={`tabpanel-${tab}`} role="tabpanel" aria-labelledby={`tab-${tab}`}>
        {tab === 'accuracy' && <AccuracyTab />}
        {tab === 'convergence' && <ConvergenceTab />}
        {tab === 'costs' && <CostTab />}
        {tab === 'wisdom' && <WisdomTab />}
      </div>
    </div>
  );
});

export default IntelligenceConsole;

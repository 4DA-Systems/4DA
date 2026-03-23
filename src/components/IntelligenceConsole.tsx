// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, useEffect, memo } from 'react';
import { cmd } from '../lib/commands';
import { AccuracyTab } from './intelligence/AccuracyTab';
import { ConvergenceTab } from './intelligence/ConvergenceTab';
import { CostTab } from './intelligence/CostTab';

// ============================================================================
// Types
// ============================================================================

type ConsoleTab = 'accuracy' | 'convergence' | 'costs';

const TABS: Array<{ id: ConsoleTab; label: string; description: string }> = [
  { id: 'accuracy', label: 'Accuracy', description: 'Scoring accuracy, tech snapshot, knowledge decay' },
  { id: 'convergence', label: 'Projects', description: 'Tech convergence, cross-project dependencies' },
  { id: 'costs', label: 'AI Costs', description: 'Usage breakdown, cost optimization' },
];

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
      className={`px-5 py-3 text-left transition-colors border-b-2 ${
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
  const [tab, setTab] = useState<ConsoleTab>('accuracy');
  const [hasAnyData, setHasAnyData] = useState<boolean | null>(null);

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
          <h3 className="text-sm font-medium text-white">Intelligence Console</h3>
          <p className="text-xs text-text-muted mt-1">
            Accuracy tracking, project convergence, and AI cost analysis.
          </p>
        </div>
        <div className="flex flex-col items-center justify-center h-64 text-center px-8">
          <div className="w-3 h-3 rounded-full bg-success/60 animate-pulse mb-4" />
          <h3 className="text-sm font-medium text-text-primary mb-2">Intelligence Growing</h3>
          <p className="text-xs text-text-muted max-w-xs">
            4DA is learning your patterns. Accuracy metrics, tech convergence,
            and cost tracking will appear here as data accumulates.
          </p>
          <p className="text-xs text-text-muted/60 mt-3">
            First insights typically appear within a week of regular use.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden flex flex-col">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border">
        <h3 className="text-sm font-medium text-white">Intelligence Console</h3>
        <p className="text-xs text-text-muted mt-1">
          Accuracy tracking, project convergence, and AI cost analysis.
        </p>
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-border" role="tablist" aria-label="Intelligence console tabs">
        {TABS.map(t => (
          <TabButton
            key={t.id}
            id={`tab-${t.id}`}
            active={tab === t.id}
            label={t.label}
            description={t.description}
            controls={`tabpanel-${t.id}`}
            onClick={() => setTab(t.id)}
          />
        ))}
      </div>

      {/* Tab content */}
      <div id={`tabpanel-${tab}`} role="tabpanel" aria-labelledby={`tab-${tab}`}>
        {tab === 'accuracy' && <AccuracyTab />}
        {tab === 'convergence' && <ConvergenceTab />}
        {tab === 'costs' && <CostTab />}
      </div>
    </div>
  );
});

export default IntelligenceConsole;

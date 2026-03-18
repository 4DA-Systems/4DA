// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useState, memo } from 'react';
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
  active,
  label,
  description,
  onClick,
}: {
  active: boolean;
  label: string;
  description: string;
  onClick: () => void;
}) {
  return (
    <button
      role="tab"
      aria-selected={active}
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
            active={tab === t.id}
            label={t.label}
            description={t.description}
            onClick={() => setTab(t.id)}
          />
        ))}
      </div>

      {/* Tab content */}
      <div role="tabpanel">
        {tab === 'accuracy' && <AccuracyTab />}
        {tab === 'convergence' && <ConvergenceTab />}
        {tab === 'costs' && <CostTab />}
      </div>
    </div>
  );
});

export default IntelligenceConsole;

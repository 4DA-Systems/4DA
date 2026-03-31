// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useGameComponent } from '../../hooks/use-game-component';

// ============================================================================
// Constants
// ============================================================================

const PHASE_CONFIG: Record<string, { label: string; color: string; bg: string }> = {
  cold_start:   { label: 'Cold Start',   color: 'text-text-muted',  bg: 'bg-text-muted/20' },
  accumulating: { label: 'Accumulating', color: 'text-amber-400',   bg: 'bg-amber-400/15' },
  compounding:  { label: 'Compounding',  color: 'text-accent-gold', bg: 'bg-accent-gold/15' },
  mature:       { label: 'Mature',       color: 'text-success',     bg: 'bg-success/15' },
};

// ============================================================================
// Sub-components
// ============================================================================

function PhaseBadge({ phase }: { phase: string }) {
  const config = PHASE_CONFIG[phase] ?? PHASE_CONFIG.cold_start!;
  return (
    <span className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${config.color} ${config.bg}`}>
      {config.label}
    </span>
  );
}

function StatCell({ value, label }: { value: string | number; label: string }) {
  return (
    <div className="text-center">
      <div className="text-lg font-semibold text-white tabular-nums">{value}</div>
      <div className="text-[10px] text-text-muted uppercase tracking-wider">{label}</div>
    </div>
  );
}

function CoverageBar({ coverage }: { coverage: number }) {
  const pct = Math.min(100, Math.max(0, coverage));
  const barColor = pct >= 90 ? 'bg-success' : pct >= 50 ? 'bg-accent-gold' : 'bg-amber-500';

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-muted uppercase tracking-wider">
          Feedback Coverage
        </span>
        <span className="text-xs text-text-secondary tabular-nums">{pct}%</span>
      </div>
      <div className="h-1.5 bg-bg-primary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${barColor}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

function PrincipleListItem({ statement, evidenceCount }: { statement: string; evidenceCount: number }) {
  const { t } = useTranslation();
  return (
    <li className="text-xs text-text-secondary leading-relaxed flex items-start gap-2">
      <span className="text-accent-gold/60 mt-0.5 flex-shrink-0">{'\u25C6'}</span>
      <span>
        {statement}
        <span className="text-text-muted ml-1">
          ({t('awe.momentum.evidenceCount', { count: evidenceCount })})
        </span>
      </span>
    </li>
  );
}

// ============================================================================
// Main Component
// ============================================================================

/**
 * MomentumWisdomTrajectory — wisdom growth trajectory with positioning.
 *
 * Shows growth phase badge, decision stats, feedback coverage,
 * pending feedback CTA, and earned principles.
 */
export const MomentumWisdomTrajectory = memo(function MomentumWisdomTrajectory() {
  const { t } = useTranslation();
  const aweSummary = useAppStore(s => s.aweSummary);
  const aweGrowthTrajectory = useAppStore(s => s.aweGrowthTrajectory);
  const loadAweSummary = useAppStore(s => s.loadAweSummary);
  const loadAweGrowthTrajectory = useAppStore(s => s.loadAweGrowthTrajectory);

  // GAME shader — must be called before any early returns (React hooks rule)
  const { containerRef: gameRef, elementRef: gameEl } = useGameComponent('game-momentum-field');

  useEffect(() => {
    void loadAweSummary();
    void loadAweGrowthTrajectory();
  }, [loadAweSummary, loadAweGrowthTrajectory]);

  const trajectory = aweGrowthTrajectory;
  const principles_formed = trajectory?.principles_formed ?? 0;
  const feedback_coverage = trajectory?.feedback_coverage ?? 0;

  // Update GAME shader params when data changes
  useEffect(() => {
    const el = gameEl.current;
    if (el) {
      el.setParam?.('principleCount', principles_formed);
      el.setParam?.('coverage', feedback_coverage / 100);
    }
  }, [principles_formed, feedback_coverage, gameEl]);

  // Don't render if AWE data isn't available
  if (!aweSummary || !aweSummary.available) return null;
  if (!trajectory) return null;

  const { growth_phase, decisions } = trajectory;
  const pendingCount = aweSummary.pending;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden relative">
      {/* GAME atmosphere layer */}
      <div ref={gameRef} className="absolute inset-0 opacity-[0.06] pointer-events-none" aria-hidden="true" />

      {/* Header */}
      <div className="relative px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-gold text-sm">{'\u25C7'}</span>
          <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
            {t('awe.momentum.title')}
          </h4>
        </div>
        <PhaseBadge phase={growth_phase} />
      </div>

      <div className="p-4 space-y-4">
        {/* Stats row */}
        <div className="grid grid-cols-3 gap-2">
          <StatCell value={decisions} label={t('awe.momentum.decisionsTracked')} />
          <StatCell value={principles_formed} label={t('awe.momentum.principlesEarned')} />
          <StatCell
            value={feedback_coverage > 0 ? `${feedback_coverage}%` : '--'}
            label={t('awe.momentum.coverageLabel')}
          />
        </div>

        {/* Coverage bar */}
        <CoverageBar coverage={feedback_coverage} />

        {/* Pending feedback CTA */}
        {feedback_coverage < 90 && pendingCount > 0 && (
          <div className="flex items-center gap-2 py-2 px-3 rounded bg-amber-500/10 border border-amber-500/20">
            <div className="w-1.5 h-1.5 rounded-full bg-amber-400 flex-shrink-0" />
            <p className="text-[10px] text-amber-400/80">
              {t('awe.momentum.pendingCta', { count: pendingCount })}
            </p>
          </div>
        )}

        {/* Principles list */}
        {aweSummary.principles > 0 && aweSummary.top_principle && (
          <div className="pt-2 border-t border-border/30">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-2">
              {t('awe.momentum.earnedPrinciples')}
            </h5>
            <ul className="space-y-1.5">
              <PrincipleListItem
                statement={aweSummary.top_principle}
                evidenceCount={aweSummary.principles}
              />
            </ul>
          </div>
        )}
      </div>
    </div>
  );
});

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

function BigMetric({ value, label }: { value: string | number; label: string }) {
  return (
    <div className="text-center">
      <div className="text-2xl font-semibold text-white tabular-nums">{value}</div>
      <div className="text-[10px] text-text-muted uppercase tracking-wider mt-1">{label}</div>
    </div>
  );
}

function CoverageBar({ coverage }: { coverage: number }) {
  const pct = Math.min(100, Math.max(0, coverage));
  const barColor = pct >= 90 ? 'bg-success' : pct >= 50 ? 'bg-accent-gold' : 'bg-amber-500';

  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-muted uppercase tracking-wider">
          Feedback Coverage
        </span>
        <span className="text-xs text-text-secondary tabular-nums">{pct}%</span>
      </div>
      <div className="h-2 bg-bg-primary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${barColor}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

/**
 * ProfileWisdomDna — developer wisdom identity and growth trajectory.
 *
 * Shows growth phase badge, decision velocity, feedback coverage,
 * and a gap-to-impact estimate.
 */
export const ProfileWisdomDna = memo(function ProfileWisdomDna() {
  const { t } = useTranslation();
  const aweSummary = useAppStore(s => s.aweSummary);
  const aweGrowthTrajectory = useAppStore(s => s.aweGrowthTrajectory);
  const loadAweSummary = useAppStore(s => s.loadAweSummary);
  const loadAweGrowthTrajectory = useAppStore(s => s.loadAweGrowthTrajectory);

  // GAME shader — must be called before any early returns (React hooks rule)
  const { containerRef: gameRef, elementRef: gameEl } = useGameComponent('game-score-fingerprint');

  useEffect(() => {
    void loadAweSummary();
    void loadAweGrowthTrajectory();
  }, [loadAweSummary, loadAweGrowthTrajectory]);

  const traj = aweGrowthTrajectory;
  const decisions = traj?.decisions ?? 0;
  const feedback_coverage = traj?.feedback_coverage ?? 0;
  const principles_formed = traj?.principles_formed ?? 0;

  // Update GAME shader params
  useEffect(() => {
    const el = gameEl.current;
    if (el) {
      el.setParam?.('relevance', Math.min(decisions / 500, 1));
      el.setParam?.('freshness', feedback_coverage / 100);
      el.setParam?.('depth', Math.min(principles_formed / 10, 1));
      el.setParam?.('confidence', feedback_coverage >= 70 ? 0.9 : 0.4);
    }
  }, [decisions, feedback_coverage, principles_formed, gameEl]);

  // Don't render if AWE data isn't available
  if (!aweSummary || !aweSummary.available) return null;
  if (!traj) return null;

  const { growth_phase } = traj;
  const phaseConfig = PHASE_CONFIG[growth_phase] ?? PHASE_CONFIG.cold_start!;
  const pendingCount = aweSummary.pending;

  // Gap-to-impact: how many resolved decisions could generate new principles
  const potentialPrinciples = pendingCount > 0 ? Math.max(1, Math.floor(pendingCount / 5)) : 0;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden relative">
      {/* GAME wisdom fingerprint */}
      <div ref={gameRef} className="absolute top-3 right-3 w-16 h-16 rounded-lg overflow-hidden opacity-50" aria-hidden="true" />

      {/* Header */}
      <div className="relative px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-gold text-sm">{'\u25C7'}</span>
          <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
            {t('awe.profile.title')}
          </h4>
        </div>
        <span className={`text-xs font-medium px-2.5 py-1 rounded-full ${phaseConfig.color} ${phaseConfig.bg}`}>
          {phaseConfig.label}
        </span>
      </div>

      <div className="p-4 space-y-5">
        {/* Big metrics row */}
        <div className="grid grid-cols-3 gap-2">
          <BigMetric value={decisions} label={t('awe.profile.decisionVelocity')} />
          <BigMetric value={principles_formed} label={t('awe.profile.principles_formed')} />
          <BigMetric
            value={feedback_coverage > 0 ? `${feedback_coverage}%` : '--'}
            label={t('awe.profile.coverage')}
          />
        </div>

        {/* Coverage bar */}
        <CoverageBar coverage={feedback_coverage} />

        {/* Gap-to-impact */}
        {potentialPrinciples > 0 && (
          <div className="flex items-center gap-2 py-2 px-3 rounded bg-accent-gold/5 border border-accent-gold/15">
            <span className="text-accent-gold text-xs">{'\u25C7'}</span>
            <p className="text-xs text-text-secondary">
              {t('awe.profile.gapToImpact', {
                pending: pendingCount,
                principles: potentialPrinciples,
              })}
            </p>
          </div>
        )}
      </div>
    </div>
  );
});

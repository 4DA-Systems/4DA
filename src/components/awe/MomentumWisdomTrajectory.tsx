// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useState, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useGameComponent } from '../../hooks/use-game-component';
// Types imported via store slice — AweBehavioralContext used structurally

// ============================================================================
// Sub-components
// ============================================================================

function VelocityIndicator({ velocity }: { velocity: number }) {
  const label = velocity > 1.5 ? 'Accelerating' : velocity > 0.8 ? 'Steady' : velocity > 0 ? 'Slowing' : 'Starting';
  const color = velocity > 1.5 ? 'text-success' : velocity > 0.8 ? 'text-text-secondary' : 'text-amber-400';
  const arrow = velocity > 1.2 ? '\u2197' : velocity > 0.8 ? '\u2192' : '\u2198';

  return (
    <div className="text-center">
      <div className={`text-lg font-semibold tabular-nums ${color}`}>{arrow} {velocity.toFixed(1)}x</div>
      <div className="text-[10px] text-text-muted uppercase tracking-wider">{label}</div>
    </div>
  );
}

function StatCell({ value, label, color }: { value: string | number; label: string; color?: string }) {
  return (
    <div className="text-center">
      <div className={`text-lg font-semibold tabular-nums ${color ?? 'text-white'}`}>{value}</div>
      <div className="text-[10px] text-text-muted uppercase tracking-wider">{label}</div>
    </div>
  );
}

function AffinityBar({ topic, score }: { topic: string; score: number }) {
  const pct = Math.abs(score) * 100;
  const isPositive = score > 0;
  return (
    <div className="flex items-center gap-2">
      <span className="text-[10px] text-text-secondary font-mono truncate w-20">{topic}</span>
      <div className="flex-1 h-1.5 bg-bg-primary rounded overflow-hidden">
        <div
          className={`h-full rounded ${isPositive ? 'bg-success' : 'bg-error'}`}
          style={{ width: `${Math.min(100, pct)}%` }}
        />
      </div>
      <span className="text-[9px] text-text-muted tabular-nums w-8 text-right">{Math.round(pct)}%</span>
    </div>
  );
}

function SourceBadge({ name, count }: { name: string; count: number }) {
  return (
    <span className="text-[10px] text-text-secondary bg-bg-tertiary rounded px-2 py-0.5 tabular-nums">
      {name} <span className="text-text-muted">{count}</span>
    </span>
  );
}

function CoverageBar({ coverage }: { coverage: number }) {
  const pct = Math.min(100, Math.max(0, coverage));
  const barColor = pct >= 50 ? 'bg-success' : pct >= 20 ? 'bg-accent-gold' : 'bg-amber-500';

  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[10px] text-text-muted uppercase tracking-wider">Feedback Coverage</span>
        <span className="text-xs text-text-secondary tabular-nums">{pct.toFixed(1)}%</span>
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

// ============================================================================
// Main Component
// ============================================================================

/**
 * MomentumWisdomTrajectory — behavioral intelligence from real 4DA data.
 *
 * Shows engagement velocity, topic affinities, source preferences,
 * feedback coverage, and LLM-synthesized wisdom — all from 4DA's
 * actual behavioral tables, not AWE's isolated git-commit database.
 */
export const MomentumWisdomTrajectory = memo(function MomentumWisdomTrajectory() {
  const { t } = useTranslation();
  const ctx = useAppStore(s => s.aweBehavioralContext);
  const wisdomSynthesis = useAppStore(s => s.aweWisdomSynthesis);
  const loadBehavioralContext = useAppStore(s => s.loadBehavioralContext);
  const synthesizeWisdom = useAppStore(s => s.synthesizeWisdom);
  const [synthRequested, setSynthRequested] = useState(false);

  // GAME shader — must be called before any early returns (React hooks rule)
  const { containerRef: gameRef, elementRef: gameEl } = useGameComponent('game-momentum-field');

  useEffect(() => {
    void loadBehavioralContext();
    // Auto-synthesize wisdom if not already loaded
    if (!wisdomSynthesis && !synthRequested) {
      setSynthRequested(true);
      void synthesizeWisdom();
    }
  }, [loadBehavioralContext, wisdomSynthesis, synthRequested, synthesizeWisdom]);

  // Update GAME shader based on real behavioral data
  useEffect(() => {
    const el = gameEl.current;
    if (el && ctx) {
      const ip = ctx.interaction_patterns;
      el.setParam?.('principleCount', ctx.topic_affinities.filter(a => a.affinity_score > 0.5).length);
      el.setParam?.('coverage', ip.total_interactions > 0 ? Math.min(1, ip.saves / Math.max(1, ip.total_interactions)) : 0);
    }
  }, [ctx, gameEl]);

  // No behavioral data yet
  if (!ctx) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-6 text-center">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin mx-auto" />
        <p className="text-xs text-text-muted mt-2">{t('awe.momentum.loading', 'Loading behavioral intelligence...')}</p>
      </div>
    );
  }

  const ip = ctx.interaction_patterns;
  const topAffinities = ctx.topic_affinities.filter(a => a.affinity_score > 0.2).slice(0, 6);
  const rejectedTopics = ctx.topic_affinities.filter(a => a.affinity_score < -0.2).slice(0, 3);
  const hasData = ip.total_interactions > 0 || topAffinities.length > 0;

  if (!hasData) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-6 text-center">
        <span className="text-accent-gold text-lg">{'\u25C7'}</span>
        <p className="text-xs text-text-muted mt-2">{t('awe.momentum.noData', 'Start engaging with content to build your behavioral profile.')}</p>
      </div>
    );
  }

  const handleSynthesize = () => {
    setSynthRequested(true);
    void synthesizeWisdom();
  };

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden relative">
      {/* GAME atmosphere */}
      <div ref={gameRef} className="absolute inset-0 opacity-[0.06] pointer-events-none" aria-hidden="true" />

      {/* Header */}
      <div className="relative px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-gold text-sm">{'\u25C7'}</span>
          <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
            {t('awe.momentum.title', 'Wisdom Trajectory')}
          </h4>
        </div>
        {ip.weekly_velocity > 1.5 && (
          <span className="text-[10px] font-medium px-2 py-0.5 rounded-full text-success bg-success/15">
            {t('awe.momentum.accelerating', 'Accelerating')}
          </span>
        )}
      </div>

      <div className="relative p-4 space-y-4">
        {/* Wisdom synthesis voice */}
        {wisdomSynthesis ? (
          <div className="py-2 px-3 rounded bg-accent-gold/5 border border-accent-gold/15">
            <p className="text-xs text-accent-gold/90 leading-relaxed">{wisdomSynthesis}</p>
          </div>
        ) : !synthRequested ? (
          <button
            onClick={handleSynthesize}
            className="w-full py-2 px-3 rounded border border-accent-gold/20 text-[10px] text-accent-gold/70 hover:bg-accent-gold/5 transition-colors"
          >
            {t('awe.momentum.synthesize', 'Synthesize wisdom from your behavioral data')}
          </button>
        ) : (
          <div className="py-2 px-3 rounded bg-bg-tertiary text-center">
            <p className="text-[10px] text-text-muted">{t('awe.momentum.synthesizing', 'Synthesizing...')}</p>
          </div>
        )}

        {/* Stats row */}
        <div className="grid grid-cols-3 gap-2">
          <StatCell value={ip.total_interactions} label={t('awe.momentum.interactions', 'Interactions')} />
          <VelocityIndicator velocity={ip.weekly_velocity} />
          <StatCell
            value={`${ip.saves}`}
            label={t('awe.momentum.saved', 'Saved')}
            color={ip.saves > 0 ? 'text-success' : undefined}
          />
        </div>

        {/* Topic affinities */}
        {topAffinities.length > 0 && (
          <div>
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
              {t('awe.momentum.strongAffinities', 'Strongest Affinities')}
            </h5>
            <div className="space-y-1.5">
              {topAffinities.map(a => (
                <AffinityBar key={a.topic} topic={a.topic} score={a.affinity_score} />
              ))}
            </div>
          </div>
        )}

        {/* Rejected topics */}
        {rejectedTopics.length > 0 && (
          <div>
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
              {t('awe.momentum.rejected', 'Consistently Rejected')}
            </h5>
            <div className="flex flex-wrap gap-1.5">
              {rejectedTopics.map(a => (
                <span key={a.topic} className="text-[10px] text-error/60 bg-error/5 rounded px-2 py-0.5 font-mono">
                  {a.topic}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Source preferences */}
        {ip.top_sources.length > 0 && (
          <div>
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
              {t('awe.momentum.topSources', 'Preferred Sources')}
            </h5>
            <div className="flex flex-wrap gap-1.5">
              {ip.top_sources.map(([name, count]) => (
                <SourceBadge key={name} name={name} count={count} />
              ))}
            </div>
          </div>
        )}

        {/* Feedback coverage */}
        <CoverageBar coverage={ctx.feedback_stats.coverage_pct} />

        {/* Advantage score */}
        {ctx.advantage_trajectory.length > 0 && (
          <div className="flex items-center justify-between pt-2 border-t border-border/30">
            <span className="text-[10px] text-text-muted uppercase tracking-wider">
              {t('awe.momentum.compoundAdvantage', 'Compound Advantage')}
            </span>
            <span className="text-sm font-semibold text-accent-gold tabular-nums">
              {ctx.advantage_trajectory[0]?.score.toFixed(1)}
            </span>
          </div>
        )}

        {/* Calibration insights */}
        {ctx.calibration_insights.length > 0 && (
          <div className="pt-2 border-t border-border/30">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
              {t('awe.momentum.calibration', 'Calibration Insights')}
            </h5>
            <ul className="space-y-1">
              {ctx.calibration_insights.slice(0, 3).map((c, i) => (
                <li key={i} className="text-[10px] text-text-secondary flex items-start gap-1.5">
                  <span className="text-accent-gold/40 mt-0.5">{'\u25C6'}</span>
                  <span>
                    <span className="text-text-muted font-mono">[{c.digest_type}]</span>{' '}
                    {c.subject}
                    <span className="text-text-muted ml-1">({Math.round(c.confidence * 100)}%, n={c.sample_size})</span>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Decision outcomes summary */}
        {ctx.decision_outcomes.length > 0 && (() => {
          const acted = ctx.decision_outcomes.filter(d => d.status === 'acted' || d.status === 'closed').length;
          const open = ctx.decision_outcomes.filter(d => d.status === 'open').length;
          const expired = ctx.decision_outcomes.filter(d => d.status === 'expired').length;
          return (
            <div className="flex items-center gap-4 pt-2 border-t border-border/30 text-[10px]">
              <span className="text-text-muted uppercase tracking-wider">Decisions</span>
              <span className="text-success tabular-nums">{acted} acted</span>
              <span className="text-text-secondary tabular-nums">{open} open</span>
              {expired > 0 && <span className="text-amber-400 tabular-nums">{expired} expired</span>}
            </div>
          );
        })()}
      </div>
    </div>
  );
});

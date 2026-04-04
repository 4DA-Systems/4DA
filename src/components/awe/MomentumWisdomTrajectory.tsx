// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useGameComponent } from '../../hooks/use-game-component';

// ============================================================================
// Sub-components — clean, human-readable, zero jargon
// ============================================================================

function BigStat({ value, label, sub, color }: { value: string | number; label: string; sub?: string; color?: string }) {
  return (
    <div className="text-center">
      <div className={`text-xl font-semibold tabular-nums ${color ?? 'text-white'}`}>{value}</div>
      <div className="text-[10px] text-text-muted mt-0.5">{label}</div>
      {sub && <div className="text-[9px] text-text-muted/60 mt-0.5">{sub}</div>}
    </div>
  );
}

function InsightRow({ icon, text, color }: { icon: string; text: string; color?: string }) {
  return (
    <div className="flex items-start gap-2 py-1.5">
      <span className={`text-xs mt-0.5 flex-shrink-0 ${color ?? 'text-accent-gold/60'}`}>{icon}</span>
      <p className="text-xs text-text-secondary leading-relaxed">{text}</p>
    </div>
  );
}

function SourceBar({ name, count, total }: { name: string; count: number; total: number }) {
  const pct = total > 0 ? (count / total) * 100 : 0;
  return (
    <div className="flex items-center gap-2">
      <span className="text-[10px] text-text-secondary w-24 truncate capitalize">{name.replace('_', ' ')}</span>
      <div className="flex-1 h-1.5 bg-bg-primary rounded overflow-hidden">
        <div className="h-full rounded bg-accent-gold/60" style={{ width: `${Math.min(100, pct)}%` }} />
      </div>
      <span className="text-[9px] text-text-muted tabular-nums w-12 text-right">{count}</span>
    </div>
  );
}

// ============================================================================
// Intelligence Insights — computed from real data, no LLM needed
// ============================================================================

function computeInsights(ctx: ReturnType<typeof useAppStore.getState>['aweBehavioralContext']) {
  if (!ctx) return [];
  const insights: Array<{ icon: string; text: string; color?: string }> = [];
  const ip = ctx.interaction_patterns;
  const ic = ctx.instant_context;

  // Intelligence volume
  if (ic.total_source_items > 1000) {
    insights.push({
      icon: '\u25C6',
      text: `Monitoring ${ic.total_source_items.toLocaleString()} items across ${ic.source_breakdown.length} sources. ${ic.items_last_24h} new in the last 24 hours.`,
    });
  } else if (ic.total_source_items > 0) {
    insights.push({
      icon: '\u25C6',
      text: `${ic.total_source_items} items gathered from ${ic.source_breakdown.length} sources. Intelligence is building — it compounds over time.`,
    });
  }

  // Engagement quality
  if (ip.total_interactions > 50 && ip.saves > 0) {
    const saveRate = Math.round((ip.saves / ip.total_interactions) * 100);
    insights.push({
      icon: '\u25C6',
      text: `You save ${saveRate}% of what you engage with. ${saveRate > 20 ? 'Highly selective — the system is learning your standards.' : 'Each save teaches the system what matters to you.'}`,
    });
  } else if (ip.total_interactions > 0 && ip.saves === 0) {
    insights.push({
      icon: '\u2192',
      text: 'Save items you find valuable — this teaches the system your preferences and sharpens future results.',
      color: 'text-amber-400/60',
    });
  }

  // Velocity trend
  if (ip.weekly_velocity > 1.5) {
    insights.push({
      icon: '\u2197',
      text: `Engagement accelerating (${ip.weekly_velocity.toFixed(1)}x vs last week). You're in a discovery phase.`,
      color: 'text-success/60',
    });
  } else if (ip.weekly_velocity < 0.5 && ip.total_interactions > 20) {
    insights.push({
      icon: '\u2198',
      text: 'Engagement slowing. The system adapts — less noise, higher signal when you return.',
      color: 'text-amber-400/60',
    });
  }

  // Topic diversity
  const strongTopics = ctx.topic_affinities.filter(a => a.affinity_score > 0.3);
  if (strongTopics.length >= 3) {
    const topNames = strongTopics.slice(0, 3).map(a => a.topic).join(', ');
    insights.push({
      icon: '\u25C6',
      text: `Strongest interests: ${topNames}. Results are tuned to these areas.`,
    });
  }

  // Source concentration warning
  if (ic.source_breakdown.length > 0) {
    const totalItems = ic.source_breakdown.reduce((sum, [, c]) => sum + c, 0);
    const topSource = ic.source_breakdown[0];
    if (topSource && totalItems > 0) {
      const topPct = Math.round((topSource[1] / totalItems) * 100);
      if (topPct > 60) {
        insights.push({
          icon: '\u26A0',
          text: `${topPct}% of intelligence comes from ${topSource[0].replace('_', ' ')}. Diversifying sources reveals blind spots.`,
          color: 'text-amber-400/60',
        });
      }
    }
  }

  // Feedback guidance
  if (ctx.feedback_stats.coverage_pct < 5 && ip.total_interactions > 0) {
    insights.push({
      icon: '\u2192',
      text: 'Mark items as relevant or not relevant to calibrate the scoring engine. Even a few signals make a difference.',
      color: 'text-text-muted',
    });
  }

  return insights;
}

// ============================================================================
// Main Component
// ============================================================================

export const MomentumWisdomTrajectory = memo(function MomentumWisdomTrajectory() {
  const { t } = useTranslation();
  const ctx = useAppStore(s => s.aweBehavioralContext);
  const loadBehavioralContext = useAppStore(s => s.loadBehavioralContext);

  const { containerRef: gameRef, elementRef: gameEl } = useGameComponent('game-momentum-field');

  useEffect(() => {
    void loadBehavioralContext();
  }, [loadBehavioralContext]);

  useEffect(() => {
    const el = gameEl.current;
    if (el && ctx) {
      const ip = ctx.interaction_patterns;
      el.setParam?.('principleCount', ctx.topic_affinities.filter(a => a.affinity_score > 0.5).length);
      el.setParam?.('coverage', ip.total_interactions > 0 ? Math.min(1, ip.saves / Math.max(1, ip.total_interactions)) : 0);
    }
  }, [ctx, gameEl]);

  if (!ctx) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-6 text-center">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin mx-auto" />
        <p className="text-xs text-text-muted mt-2">Loading intelligence...</p>
      </div>
    );
  }

  const ip = ctx.interaction_patterns;
  const ic = ctx.instant_context;
  const insights = computeInsights(ctx);

  // Source data — prefer interaction sources, fallback to instant context
  const sources = ip.top_sources.length > 0 ? ip.top_sources : ic.source_breakdown;
  const totalSourceItems = sources.reduce((sum, [, c]) => sum + c, 0);

  // Strong affinities only (>30% and >10 exposures to avoid noise)
  const affinities = ctx.topic_affinities
    .filter(a => a.affinity_score > 0.3 && a.total_exposures >= 10)
    .slice(0, 5);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden relative">
      <div ref={gameRef} className="absolute inset-0 opacity-[0.06] pointer-events-none" aria-hidden="true" />

      {/* Header */}
      <div className="relative px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-gold text-sm">{'\u25C7'}</span>
          <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
            {t('awe.momentum.title', 'Intelligence Profile')}
          </h4>
        </div>
        <span className={`text-[10px] px-2 py-0.5 rounded-full ${
          ic.data_level === 'rich' ? 'text-success bg-success/15' :
          ic.data_level === 'warming' ? 'text-amber-400 bg-amber-400/15' :
          'text-text-muted bg-text-muted/15'
        }`}>
          {ic.total_source_items.toLocaleString()} items
        </span>
      </div>

      <div className="relative p-4 space-y-4">
        {/* Key metrics — every number has context */}
        <div className="grid grid-cols-3 gap-3">
          <BigStat
            value={ic.total_source_items.toLocaleString()}
            label="Items Monitored"
            sub={ic.items_last_24h > 0 ? `+${ic.items_last_24h} new today` : 'from all sources'}
          />
          <BigStat
            value={sources.length}
            label={sources.length === 1 ? 'Source Active' : 'Sources Active'}
            sub={sources.length >= 5 ? 'Good diversity' : sources.length >= 3 ? 'Growing' : 'Add more in settings'}
          />
          <BigStat
            value={ip.total_interactions > 0 ? ip.total_interactions : '--'}
            label="Your Actions"
            sub={ip.total_interactions > 0
              ? `${ip.saves} saved, ${ip.dismissals} dismissed`
              : 'Click, save, or dismiss items'}
            color={ip.total_interactions > 0 ? 'text-success' : undefined}
          />
        </div>

        {/* Computed insights — no LLM, no hallucination, just data */}
        {insights.length > 0 && (
          <div className="border-t border-border/30 pt-3">
            {insights.map((insight, i) => (
              <InsightRow key={i} icon={insight.icon} text={insight.text} color={insight.color} />
            ))}
          </div>
        )}

        {/* Source distribution */}
        {sources.length > 0 && (
          <div className="border-t border-border/30 pt-3">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
              Where your intelligence comes from
            </h5>
            <p className="text-[9px] text-text-muted/60 mb-2">
              4DA scans these sources automatically. A balanced mix reduces blind spots.
            </p>
            <div className="space-y-1.5">
              {sources.slice(0, 6).map(([name, count]) => (
                <SourceBar key={name} name={name} count={count} total={totalSourceItems} />
              ))}
            </div>
          </div>
        )}

        {/* Topic affinities — only show strong, validated ones */}
        {affinities.length > 0 && (
          <div className="border-t border-border/30 pt-3">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
              Topics you engage with most
            </h5>
            <p className="text-[9px] text-text-muted/60 mb-2">
              Learned from your saves, clicks, and dismissals. Numbers show total exposures.
            </p>
            <div className="flex flex-wrap gap-1.5">
              {affinities.map(a => (
                <span key={a.topic} className="text-[10px] text-accent-gold/80 bg-accent-gold/8 border border-accent-gold/15 rounded-full px-2.5 py-0.5">
                  {a.topic}
                  <span className="text-accent-gold/40 ml-1">{a.total_exposures}</span>
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Compound advantage — with explanation */}
        {ctx.advantage_trajectory.length > 0 && ctx.advantage_trajectory[0] != null && ctx.advantage_trajectory[0].score > 0 && (() => {
          const score = ctx.advantage_trajectory[0]!.score;
          const level = score >= 70 ? 'Excellent' : score >= 40 ? 'Building' : score >= 20 ? 'Early' : 'Starting';
          const levelColor = score >= 70 ? 'text-success' : score >= 40 ? 'text-accent-gold' : 'text-text-secondary';
          return (
            <div className="pt-3 border-t border-border/30 space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-[10px] text-text-muted uppercase tracking-wider">
                  Advantage Score
                </span>
                <div className="flex items-center gap-2">
                  <span className={`text-[10px] ${levelColor}`}>{level}</span>
                  <span className="text-sm font-semibold text-accent-gold tabular-nums">{score.toFixed(0)}</span>
                </div>
              </div>
              <p className="text-[10px] text-text-muted/70 leading-relaxed">
                Measures how early you see important signals compared to acting on them. Higher means 4DA is surfacing relevant intelligence before you need it.
              </p>
            </div>
          );
        })()}
      </div>
    </div>
  );
});

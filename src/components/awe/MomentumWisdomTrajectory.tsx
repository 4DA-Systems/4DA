// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useGameComponent } from '../../hooks/use-game-component';
import type { AweSummary, AweWisdomWell, AwePendingDecision, AweBehavioralContext } from '../../types/awe';

// ============================================================================
// Sub-components
// ============================================================================

function BigStat({ value, label, sub, color }: { value: string | number; label: string; sub?: string; color?: string }) {
  return (
    <div className="text-center">
      <div className={`text-xl font-semibold tabular-nums ${color ?? 'text-white'}`}>{value}</div>
      <div className="text-[10px] text-text-muted mt-0.5">{label}</div>
      {sub != null && sub.length > 0 && (
        <div className="text-[9px] text-text-muted/60 mt-0.5">{sub}</div>
      )}
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
// Wisdom Phase — classifies AWE state from summary + well data
// ============================================================================

type WisdomPhase = 'empty' | 'seeding' | 'learning' | 'calibrating' | 'compounding' | 'mature';

interface WisdomState {
  phase: WisdomPhase;
  phaseLabel: string;
  phaseColor: string;
  narrative: string;
}

function classifyPhase(summary: AweSummary | null, well: AweWisdomWell | null): WisdomState {
  if (!summary || !summary.available) {
    return {
      phase: 'empty',
      phaseLabel: 'Offline',
      phaseColor: 'text-text-muted',
      narrative: 'AWE engine not connected. Wisdom is computed from your decisions, feedback, and outcomes — see Settings.',
    };
  }

  const { decisions, principles, feedback_coverage, feedback_count } = summary;
  const wellSize = well != null
    ? (well.surface.length + well.pattern.length + well.principle.length
       + well.causal.length + well.meta.length + well.universal.length)
    : 0;

  if (decisions < 5) {
    return {
      phase: 'seeding',
      phaseLabel: 'Seeding',
      phaseColor: 'text-text-muted',
      narrative: `${decisions} decision${decisions === 1 ? '' : 's'} tracked. Wisdom begins forming after 5+ decisions with outcome feedback.`,
    };
  }

  if (feedback_coverage < 30) {
    return {
      phase: 'learning',
      phaseLabel: 'Learning',
      phaseColor: 'text-amber-400',
      narrative: `${decisions} decisions tracked but only ${feedback_coverage}% have outcome feedback. Principles cannot validate without evidence — review the pending queue.`,
    };
  }

  if (principles === 0) {
    return {
      phase: 'calibrating',
      phaseLabel: 'Calibrating',
      phaseColor: 'text-amber-400',
      narrative: `${decisions} decisions, ${feedback_count} feedback signals recorded (${feedback_coverage}% coverage). Patterns are forming but no principle has crossed the validation threshold yet.`,
    };
  }

  if (principles < 3 || wellSize < 10) {
    return {
      phase: 'compounding',
      phaseLabel: 'Compounding',
      phaseColor: 'text-accent-gold',
      narrative: `${principles} principle${principles === 1 ? '' : 's'} validated from ${decisions} decisions. The wisdom graph is growing — keep providing outcome feedback to compound.`,
    };
  }

  return {
    phase: 'mature',
    phaseLabel: 'Mature',
    phaseColor: 'text-success',
    narrative: `${principles} principles validated, ${decisions} decisions tracked, ${feedback_coverage}% feedback coverage. AWE is actively calibrated and shaping 4DA's judgment.`,
  };
}

// ============================================================================
// Insights — computed from real AWE + behavioral data (no LLM, no hallucination)
// ============================================================================

interface Insight { icon: string; text: string; color?: string }

function computeInsights(
  summary: AweSummary | null,
  pending: AwePendingDecision[],
  ctx: AweBehavioralContext | null,
): Insight[] {
  const out: Insight[] = [];

  // Pending feedback — actionable
  if (summary?.available === true && pending.length > 0) {
    const oldestDays = Math.max(...pending.map(p => p.age_days));
    out.push({
      icon: '\u25C6',
      text: `${pending.length} decision${pending.length === 1 ? '' : 's'} awaiting outcome feedback${oldestDays > 7 ? ` (oldest: ${oldestDays} days)` : ''}. Each resolved decision sharpens the wisdom graph.`,
      color: 'text-amber-400/70',
    });
  }

  // Top principle — the crown jewel
  if (summary?.top_principle != null && summary.top_principle.length > 0) {
    out.push({
      icon: '\u2737',
      text: `Top principle: ${summary.top_principle}`,
      color: 'text-accent-gold',
    });
  }

  // Calibration health guidance
  if (summary?.available === true) {
    switch (summary.health) {
      case 'cold':
        out.push({
          icon: '\u2192',
          text: 'Cold start — no decisions recorded yet. AWE compounds as you make and evaluate choices.',
          color: 'text-text-muted',
        });
        break;
      case 'needs_feedback':
        out.push({
          icon: '\u2197',
          text: `Feedback coverage is the bottleneck (${summary.feedback_coverage}%). Without outcomes, decisions cannot crystallize into principles.`,
          color: 'text-amber-400/70',
        });
        break;
      case 'learning':
        out.push({
          icon: '\u25C6',
          text: `Learning phase — ${summary.decisions} decisions, ${summary.feedback_coverage}% coverage. Patterns emerging.`,
        });
        break;
      case 'good':
      case 'healthy':
        if (summary.principles > 0) {
          out.push({
            icon: '\u25C6',
            text: `Wisdom is compounding — ${summary.principles} validated principle${summary.principles === 1 ? '' : 's'} actively shape scoring and recommendations.`,
            color: 'text-success/70',
          });
        }
        break;
    }
  }

  // Behavioral velocity (secondary — still useful if present)
  if (ctx != null) {
    const ip = ctx.interaction_patterns;
    if (ip.weekly_velocity > 1.5) {
      out.push({
        icon: '\u2197',
        text: `Engagement accelerating (${ip.weekly_velocity.toFixed(1)}x vs last week). Discovery phase — AWE will see more signals to work with.`,
        color: 'text-success/60',
      });
    }
  }

  return out;
}

// ============================================================================
// Main Component
// ============================================================================

export const MomentumWisdomTrajectory = memo(function MomentumWisdomTrajectory() {
  const { t } = useTranslation();

  // Real AWE data — the flagship source
  const aweSummary = useAppStore(s => s.aweSummary);
  const aweWisdomWell = useAppStore(s => s.aweWisdomWell);
  const awePendingDecisions = useAppStore(s => s.awePendingDecisions);
  const loadAweSummary = useAppStore(s => s.loadAweSummary);
  const loadAweWisdomWell = useAppStore(s => s.loadAweWisdomWell);
  const loadAwePendingDecisions = useAppStore(s => s.loadAwePendingDecisions);

  // Behavioral context — secondary (interaction patterns, topic affinities)
  const ctx = useAppStore(s => s.aweBehavioralContext);
  const loadBehavioralContext = useAppStore(s => s.loadBehavioralContext);

  const { containerRef: gameRef, elementRef: gameEl } = useGameComponent('game-momentum-field');

  useEffect(() => {
    void loadAweSummary();
    void loadAweWisdomWell();
    void loadAwePendingDecisions(20);
    void loadBehavioralContext();
  }, [loadAweSummary, loadAweWisdomWell, loadAwePendingDecisions, loadBehavioralContext]);

  // Drive the background shader from REAL wisdom signals, not interaction proxies
  useEffect(() => {
    const el = gameEl.current;
    if (!el) return;
    if (aweSummary?.available === true) {
      el.setParam?.('principleCount', aweSummary.principles);
      el.setParam?.('coverage', aweSummary.feedback_coverage / 100);
    } else if (ctx) {
      const ip = ctx.interaction_patterns;
      el.setParam?.('principleCount', ctx.topic_affinities.filter(a => a.affinity_score > 0.5).length);
      el.setParam?.('coverage', ip.total_interactions > 0 ? Math.min(1, ip.saves / Math.max(1, ip.total_interactions)) : 0);
    }
  }, [aweSummary, ctx, gameEl]);

  // Classify wisdom phase — drives headline narrative
  // All hooks must run on every render — keep them BEFORE any early return.
  const phaseState = useMemo(
    () => classifyPhase(aweSummary, aweWisdomWell),
    [aweSummary, aweWisdomWell],
  );

  const insights = useMemo(
    () => computeInsights(aweSummary, awePendingDecisions, ctx),
    [aweSummary, awePendingDecisions, ctx],
  );

  // Recent well highlights — the actual crystallizing intelligence.
  // Must be declared before the loading early-return (Hooks rule).
  const wellHighlights = useMemo(() => {
    if (aweWisdomWell == null) return [];
    const pools = [
      ...aweWisdomWell.principle.map(x => ({ ...x, layer: 'principle' })),
      ...aweWisdomWell.causal.map(x => ({ ...x, layer: 'causal' })),
      ...aweWisdomWell.pattern.map(x => ({ ...x, layer: 'pattern' })),
    ];
    return pools
      .sort((a, b) => b.confidence - a.confidence)
      .slice(0, 3);
  }, [aweWisdomWell]);

  // Loading: wait for at least the summary probe to return
  if (aweSummary == null && ctx == null) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-6 text-center">
        <div className="w-5 h-5 border-2 border-gray-600 border-t-white rounded-full animate-spin mx-auto" />
        <p className="text-xs text-text-muted mt-2">{t('awe.momentum.loading', 'Loading wisdom trajectory...')}</p>
      </div>
    );
  }

  const hasWisdom = aweSummary?.available === true;

  // Behavioral sources (secondary — only if present)
  const ip = ctx?.interaction_patterns ?? null;
  const sources = ip != null && ip.top_sources.length > 0 ? ip.top_sources : ctx?.instant_context.source_breakdown ?? [];
  const totalSourceItems = sources.reduce((sum, [, c]) => sum + c, 0);

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden relative">
      <div ref={gameRef} className="absolute inset-0 opacity-[0.06] pointer-events-none" aria-hidden="true" />

      {/* Header — now includes live phase badge */}
      <div className="relative px-4 py-3 border-b border-border/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-accent-gold text-sm">{'\u25C7'}</span>
          <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
            {t('awe.momentum.title', 'Wisdom Trajectory')}
          </h4>
        </div>
        <span
          className={`text-[10px] px-2 py-0.5 rounded-full bg-bg-primary/60 border border-border/30 ${phaseState.phaseColor}`}
          title={phaseState.narrative}
        >
          {phaseState.phaseLabel}
        </span>
      </div>

      <div className="relative p-4 space-y-4">
        {/* Phase narrative — the headline */}
        <p className="text-[11px] text-text-secondary leading-relaxed italic">
          {phaseState.narrative}
        </p>

        {/* Hero — REAL wisdom metrics, not behavioral proxies */}
        {hasWisdom ? (
          <div className="grid grid-cols-3 gap-3">
            <BigStat
              value={aweSummary?.decisions ?? 0}
              label="Decisions Tracked"
              sub={aweSummary != null && aweSummary.decisions > 0 ? `${aweSummary.feedback_count} with feedback` : 'none yet'}
              color={aweSummary != null && aweSummary.decisions > 0 ? 'text-white' : 'text-text-muted'}
            />
            <BigStat
              value={`${aweSummary?.feedback_coverage ?? 0}%`}
              label="Feedback Coverage"
              sub={
                (aweSummary?.feedback_coverage ?? 0) >= 70 ? 'calibrating well'
                : (aweSummary?.feedback_coverage ?? 0) >= 30 ? 'needs more outcomes'
                : 'bottleneck — record outcomes'
              }
              color={
                (aweSummary?.feedback_coverage ?? 0) >= 70 ? 'text-success'
                : (aweSummary?.feedback_coverage ?? 0) >= 30 ? 'text-amber-400'
                : 'text-text-muted'
              }
            />
            <BigStat
              value={aweSummary?.principles ?? 0}
              label={aweSummary?.principles === 1 ? 'Validated Principle' : 'Validated Principles'}
              sub={
                (aweSummary?.principles ?? 0) > 0 ? 'shape 4DA scoring'
                : (aweSummary?.pending ?? 0) > 0 ? `${aweSummary?.pending} pending review`
                : 'emerge from outcomes'
              }
              color={(aweSummary?.principles ?? 0) > 0 ? 'text-accent-gold' : 'text-text-muted'}
            />
          </div>
        ) : (
          <div className="bg-bg-tertiary/40 rounded-lg border border-border/30 p-4 text-center">
            <p className="text-xs text-text-secondary">
              {t('awe.momentum.offline', 'The wisdom engine (AWE) is not yet connected. Its role: transmute your decisions and their outcomes into calibrated principles that steer 4DA.')}
            </p>
            <p className="text-[10px] text-text-muted mt-2">
              {t('awe.momentum.offlineHint', 'See the Wisdom Tab for setup.')}
            </p>
          </div>
        )}

        {/* Insights — actionable, data-grounded */}
        {insights.length > 0 && (
          <div className="border-t border-border/30 pt-3">
            {insights.map((insight, i) => (
              <InsightRow key={i} icon={insight.icon} text={insight.text} color={insight.color} />
            ))}
          </div>
        )}

        {/* Wisdom Well highlights — the crystallizing intelligence */}
        {wellHighlights.length > 0 && (
          <div className="border-t border-border/30 pt-3">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-1">
              {t('awe.momentum.wellHighlights', 'Forming intelligence')}
            </h5>
            <p className="text-[9px] text-text-muted/60 mb-2">
              {t('awe.momentum.wellHighlightsHint', 'The highest-confidence patterns currently in the Wisdom Well.')}
            </p>
            <ul className="space-y-1.5">
              {wellHighlights.map((item, i) => (
                <li key={i} className="flex items-start gap-2">
                  <span className="text-[10px] text-accent-gold/60 mt-0.5 flex-shrink-0">{'\u25C6'}</span>
                  <div className="flex-1 min-w-0">
                    <p className="text-[11px] text-text-secondary leading-relaxed">{item.statement}</p>
                    <div className="flex items-center gap-2 mt-0.5">
                      <span className="text-[9px] text-text-muted uppercase tracking-wider">{item.layer}</span>
                      <span className="text-[9px] text-accent-gold/60 tabular-nums">{Math.round(item.confidence * 100)}%</span>
                      {item.evidence_count > 0 && (
                        <span className="text-[9px] text-text-muted/60">
                          {t('awe.momentum.evidenceCount', '{{count}} evidence', { count: item.evidence_count })}
                        </span>
                      )}
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Pending queue — direct call to action */}
        {awePendingDecisions.length > 0 && (
          <div className="border-t border-border/30 pt-3">
            <div className="flex items-center justify-between">
              <h5 className="text-[10px] text-amber-400/80 uppercase tracking-wider">
                {t('awe.momentum.pendingTitle', 'Awaiting outcome feedback')}
              </h5>
              <span className="text-[10px] text-amber-400/60 tabular-nums">{awePendingDecisions.length}</span>
            </div>
            <ul className="mt-1.5 space-y-0.5">
              {awePendingDecisions.slice(0, 3).map(d => (
                <li key={d.id} className="flex items-start gap-2">
                  <span className="text-[10px] text-amber-400/40 mt-0.5 flex-shrink-0">{'\u25B8'}</span>
                  <p className="text-[11px] text-text-secondary leading-relaxed truncate flex-1">{d.statement}</p>
                  <span className="text-[9px] text-text-muted/60 flex-shrink-0">
                    {t('awe.momentum.daysAgo', '{{count}}d', { count: d.age_days })}
                  </span>
                </li>
              ))}
            </ul>
            {awePendingDecisions.length > 3 && (
              <p className="text-[10px] text-text-muted/60 mt-1 text-center">
                {t('awe.momentum.pendingMore', '+{{count}} more in the Wisdom Tab', { count: awePendingDecisions.length - 3 })}
              </p>
            )}
          </div>
        )}

        {/* Secondary panel: Intelligence Profile (what was primary before) */}
        {ctx != null && (
          <div className="border-t border-border/30 pt-3">
            <h5 className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
              {t('awe.momentum.intelligenceProfile', 'Intelligence profile')}
            </h5>

            {/* Monitoring stats — now clearly secondary, no longer pretending to be wisdom */}
            <div className="grid grid-cols-3 gap-2 mb-3">
              <div className="bg-bg-primary/40 rounded px-2 py-1.5 text-center">
                <div className="text-[13px] font-medium text-text-secondary tabular-nums">
                  {ctx.instant_context.total_source_items.toLocaleString()}
                </div>
                <div className="text-[9px] text-text-muted">{t('awe.momentum.itemsMonitored', 'items monitored')}</div>
              </div>
              <div className="bg-bg-primary/40 rounded px-2 py-1.5 text-center">
                <div className="text-[13px] font-medium text-text-secondary tabular-nums">
                  {ctx.instant_context.source_breakdown.length}
                </div>
                <div className="text-[9px] text-text-muted">{t('awe.momentum.sourcesActive', 'sources producing')}</div>
              </div>
              <div className="bg-bg-primary/40 rounded px-2 py-1.5 text-center">
                <div className="text-[13px] font-medium text-text-secondary tabular-nums">
                  {ctx.interaction_patterns.total_interactions || '\u2014'}
                </div>
                <div className="text-[9px] text-text-muted">{t('awe.momentum.yourActions', 'your actions')}</div>
              </div>
            </div>

            {/* Source distribution — only if there is real data */}
            {sources.length > 0 && totalSourceItems > 0 && (
              <div className="space-y-1">
                {sources.slice(0, 4).map(([name, count]) => (
                  <SourceBar key={name} name={name} count={count} total={totalSourceItems} />
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
});

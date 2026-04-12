// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Shared sub-components and helpers for MomentumWisdomTrajectory.
 * Extracted to keep the main component under the 500-line file size limit.
 */

import type { AweSummary, AweWisdomWell, AwePendingDecision, AweBehavioralContext } from '../../types/awe';

// ============================================================================
// Sub-components
// ============================================================================

export function BigStat({ value, label, sub, color }: { value: string | number; label: string; sub?: string; color?: string }) {
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

export function InsightRow({ icon, text, color }: { icon: string; text: string; color?: string }) {
  return (
    <div className="flex items-start gap-2 py-1.5">
      <span className={`text-xs mt-0.5 flex-shrink-0 ${color ?? 'text-accent-gold/60'}`}>{icon}</span>
      <p className="text-xs text-text-secondary leading-relaxed">{text}</p>
    </div>
  );
}

export function SourceBar({ name, count, total }: { name: string; count: number; total: number }) {
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

export type WisdomPhase = 'empty' | 'seeding' | 'learning' | 'calibrating' | 'compounding' | 'mature';

export interface WisdomState {
  phase: WisdomPhase;
  phaseLabel: string;
  phaseColor: string;
  narrative: string;
}

export function classifyPhase(summary: AweSummary | null, well: AweWisdomWell | null): WisdomState {
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

export interface Insight { icon: string; text: string; color?: string }

export function computeInsights(
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

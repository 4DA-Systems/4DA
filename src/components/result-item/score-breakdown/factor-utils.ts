import type { ScoreBreakdown } from '../../../types';

export interface Factor {
  key: string;
  /** i18n key for the factor label */
  labelKey: string;
  /** Fallback label (English) */
  label: string;
  value: number;
  /** 'boost' = positive contribution, 'penalty' = reduced score, 'neutral' = no effect */
  effect: 'boost' | 'penalty' | 'neutral';
  /** Display format: 'score' (0-100%), 'mult' (x1.2), 'raw' (+0.15) */
  format: 'score' | 'mult' | 'raw';
  /** Maximum possible value for bar width calculation */
  max: number;
  /** Extra detail text */
  detail?: string;
}

export function extractFactors(b: ScoreBreakdown): Factor[] {
  const factors: Factor[] = [];

  if (b.context_score > 0) {
    factors.push({
      key: 'context', labelKey: 'scoreDrawer.factor.context', label: 'Project context', value: b.context_score,
      effect: b.context_score > 0.3 ? 'boost' : 'neutral', format: 'score', max: 1,
    });
  }
  if (b.interest_score > 0) {
    factors.push({
      key: 'interest', labelKey: 'scoreDrawer.factor.interest', label: 'Interest match', value: b.interest_score,
      effect: b.interest_score > 0.3 ? 'boost' : 'neutral', format: 'score', max: 1,
    });
  }
  if ((b.dep_match_score ?? 0) > 0) {
    factors.push({
      key: 'dependency', labelKey: 'scoreDrawer.factor.dependency', label: 'Dependency match', value: b.dep_match_score ?? 0,
      effect: 'boost', format: 'score', max: 1,
      detail: b.matched_deps?.slice(0, 3).join(', '),
    });
  }
  if (b.ace_boost > 0) {
    factors.push({
      key: 'ace', labelKey: 'scoreDrawer.factor.ace', label: 'ACE context boost', value: b.ace_boost,
      effect: 'boost', format: 'raw', max: 0.5,
    });
  }
  if ((b.intent_boost ?? 0) > 0) {
    factors.push({
      key: 'intent', labelKey: 'scoreDrawer.factor.intent', label: 'Active work match', value: b.intent_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.25,
    });
  }
  if ((b.skill_gap_boost ?? 0) > 0) {
    factors.push({
      key: 'skill_gap', labelKey: 'scoreDrawer.factor.skillGap', label: 'Skill gap', value: b.skill_gap_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.20,
    });
  }
  if ((b.stack_boost ?? 0) > 0) {
    factors.push({
      key: 'stack', labelKey: 'scoreDrawer.factor.stack', label: 'Stack pain point', value: b.stack_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.20,
    });
  }
  if ((b.window_boost ?? 0) > 0) {
    factors.push({
      key: 'window', labelKey: 'scoreDrawer.factor.window', label: 'Decision window', value: b.window_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.20,
    });
  }
  if ((b.feedback_boost ?? 0) !== 0) {
    const fb = b.feedback_boost ?? 0;
    factors.push({
      key: 'feedback', labelKey: 'scoreDrawer.factor.feedback', label: 'Learned preference', value: Math.abs(fb),
      effect: fb > 0 ? 'boost' : 'penalty', format: 'raw', max: 0.20,
    });
  }

  // Multipliers
  if ((b.freshness_mult ?? 1) !== 1) {
    const f = b.freshness_mult ?? 1;
    factors.push({
      key: 'freshness', labelKey: 'scoreDrawer.factor.freshness', label: 'Freshness', value: f,
      effect: f > 1 ? 'boost' : f < 0.95 ? 'penalty' : 'neutral', format: 'mult', max: 1.15,
    });
  }
  if ((b.content_quality_mult ?? 1) !== 1) {
    const q = b.content_quality_mult ?? 1;
    factors.push({
      key: 'quality', labelKey: 'scoreDrawer.factor.quality', label: 'Content quality', value: q,
      effect: q > 1 ? 'boost' : q < 0.9 ? 'penalty' : 'neutral', format: 'mult', max: 1.3,
    });
  }
  if ((b.novelty_mult ?? 1) !== 1) {
    const n = b.novelty_mult ?? 1;
    factors.push({
      key: 'novelty', labelKey: 'scoreDrawer.factor.novelty', label: 'Novelty', value: n,
      effect: n > 1 ? 'boost' : n < 0.9 ? 'penalty' : 'neutral', format: 'mult', max: 1.15,
    });
  }
  if ((b.domain_relevance ?? 1) < 0.95) {
    factors.push({
      key: 'domain', labelKey: 'scoreDrawer.factor.domain', label: 'Domain relevance', value: b.domain_relevance ?? 1,
      effect: (b.domain_relevance ?? 1) < 0.8 ? 'penalty' : 'neutral', format: 'mult', max: 1.1,
    });
  }
  if ((b.competing_mult ?? 1) < 0.95) {
    factors.push({
      key: 'competing', labelKey: 'scoreDrawer.factor.competing', label: 'Competing tech penalty', value: b.competing_mult ?? 1,
      effect: 'penalty', format: 'mult', max: 1,
    });
  }
  if (b.affinity_mult > 1.05) {
    factors.push({
      key: 'affinity', labelKey: 'scoreDrawer.factor.affinity', label: 'Topic affinity', value: b.affinity_mult,
      effect: 'boost', format: 'mult', max: 1.7,
    });
  }
  if (b.anti_penalty < 0.95) {
    factors.push({
      key: 'anti', labelKey: 'scoreDrawer.factor.anti', label: 'Anti-topic penalty', value: b.anti_penalty,
      effect: 'penalty', format: 'mult', max: 1,
    });
  }
  if ((b.confirmation_mult ?? 1) !== 1) {
    factors.push({
      key: 'confirmation', labelKey: 'scoreDrawer.factor.confirmation', label: 'Signal confirmation gate', value: b.confirmation_mult ?? 1,
      effect: (b.confirmation_mult ?? 1) > 1 ? 'boost' : 'penalty', format: 'mult', max: 1.25,
    });
  }

  return factors;
}

export function formatFactorValue(f: Factor): string {
  if (f.format === 'score') return `${Math.round(f.value * 100)}%`;
  if (f.format === 'mult') return `x${f.value.toFixed(2)}`;
  return `+${(f.value * 100).toFixed(0)}%`;
}

export function getBarWidth(f: Factor): number {
  if (f.format === 'mult') {
    // For multipliers, center at 1.0: <1.0 = penalty, >1.0 = boost
    return Math.min(Math.abs(f.value - 1.0) / 0.3 * 100, 100);
  }
  return Math.min((f.value / f.max) * 100, 100);
}

export const EFFECT_COLORS = {
  boost: { bar: 'bg-green-500/60', text: 'text-green-400', label: 'text-green-300' },
  penalty: { bar: 'bg-amber-500/60', text: 'text-amber-400', label: 'text-amber-300' },
  neutral: { bar: 'bg-gray-500/40', text: 'text-gray-400', label: 'text-gray-300' },
};

export const FACTOR_DESCRIPTIONS: Record<string, string> = {
  context: 'context matching',
  interest: 'interest relevance',
  dependency: 'dependency match',
  ace: 'project context boost',
  intent: 'work intent boost',
  skill_gap: 'skill gap detection',
  stack: 'stack relevance',
  window: 'decision window boost',
  feedback: 'learned preference',
  freshness: 'freshness weighting',
  quality: 'content quality',
  novelty: 'novelty scoring',
  domain: 'domain relevance',
  affinity: 'topic affinity',
  anti: 'anti-topic penalty',
  competing: 'competing tech penalty',
  confirmation: 'signal confirmation',
};

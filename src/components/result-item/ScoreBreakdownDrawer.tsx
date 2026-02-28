import { memo, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { ScoreBreakdown } from '../../types';

interface ScoreBreakdownDrawerProps {
  breakdown: ScoreBreakdown;
  finalScore: number;
  itemId: number;
  onClose: () => void;
  /** Optional second breakdown for comparison mode */
  compareBreakdown?: ScoreBreakdown;
  compareScore?: number;
  compareTitle?: string;
}

interface Factor {
  key: string;
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

function extractFactors(b: ScoreBreakdown): Factor[] {
  const factors: Factor[] = [];

  if (b.context_score > 0) {
    factors.push({
      key: 'context', label: 'Project context', value: b.context_score,
      effect: b.context_score > 0.3 ? 'boost' : 'neutral', format: 'score', max: 1,
    });
  }
  if (b.interest_score > 0) {
    factors.push({
      key: 'interest', label: 'Interest match', value: b.interest_score,
      effect: b.interest_score > 0.3 ? 'boost' : 'neutral', format: 'score', max: 1,
    });
  }
  if ((b.dep_match_score ?? 0) > 0) {
    factors.push({
      key: 'dependency', label: 'Dependency match', value: b.dep_match_score ?? 0,
      effect: 'boost', format: 'score', max: 1,
      detail: b.matched_deps?.slice(0, 3).join(', '),
    });
  }
  if (b.ace_boost > 0) {
    factors.push({
      key: 'ace', label: 'ACE context boost', value: b.ace_boost,
      effect: 'boost', format: 'raw', max: 0.5,
    });
  }
  if ((b.intent_boost ?? 0) > 0) {
    factors.push({
      key: 'intent', label: 'Active work match', value: b.intent_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.25,
    });
  }
  if ((b.skill_gap_boost ?? 0) > 0) {
    factors.push({
      key: 'skill_gap', label: 'Skill gap', value: b.skill_gap_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.20,
    });
  }
  if ((b.skill_gap_boost ?? 0) > 0) {
    factors.push({
      key: 'stack', label: 'Stack pain point', value: b.skill_gap_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.20,
    });
  }
  if ((b.window_boost ?? 0) > 0) {
    factors.push({
      key: 'window', label: 'Decision window', value: b.window_boost ?? 0,
      effect: 'boost', format: 'raw', max: 0.20,
    });
  }
  if ((b.feedback_boost ?? 0) !== 0) {
    const fb = b.feedback_boost ?? 0;
    factors.push({
      key: 'feedback', label: 'Learned preference', value: Math.abs(fb),
      effect: fb > 0 ? 'boost' : 'penalty', format: 'raw', max: 0.20,
    });
  }

  // Multipliers
  if ((b.freshness_mult ?? 1) !== 1) {
    const f = b.freshness_mult ?? 1;
    factors.push({
      key: 'freshness', label: 'Freshness', value: f,
      effect: f > 1 ? 'boost' : f < 0.95 ? 'penalty' : 'neutral', format: 'mult', max: 1.15,
    });
  }
  if ((b.content_quality_mult ?? 1) !== 1) {
    const q = b.content_quality_mult ?? 1;
    factors.push({
      key: 'quality', label: 'Content quality', value: q,
      effect: q > 1 ? 'boost' : q < 0.9 ? 'penalty' : 'neutral', format: 'mult', max: 1.3,
    });
  }
  if ((b.novelty_mult ?? 1) !== 1) {
    const n = b.novelty_mult ?? 1;
    factors.push({
      key: 'novelty', label: 'Novelty', value: n,
      effect: n > 1 ? 'boost' : n < 0.9 ? 'penalty' : 'neutral', format: 'mult', max: 1.15,
    });
  }
  if ((b.domain_relevance ?? 1) < 0.95) {
    factors.push({
      key: 'domain', label: 'Domain relevance', value: b.domain_relevance ?? 1,
      effect: (b.domain_relevance ?? 1) < 0.8 ? 'penalty' : 'neutral', format: 'mult', max: 1.1,
    });
  }
  if ((b.competing_mult ?? 1) < 0.95) {
    factors.push({
      key: 'competing', label: 'Competing tech penalty', value: b.competing_mult ?? 1,
      effect: 'penalty', format: 'mult', max: 1,
    });
  }
  if (b.affinity_mult > 1.05) {
    factors.push({
      key: 'affinity', label: 'Topic affinity', value: b.affinity_mult,
      effect: 'boost', format: 'mult', max: 1.7,
    });
  }
  if (b.anti_penalty < 0.95) {
    factors.push({
      key: 'anti', label: 'Anti-topic penalty', value: b.anti_penalty,
      effect: 'penalty', format: 'mult', max: 1,
    });
  }
  if ((b.confirmation_mult ?? 1) !== 1) {
    factors.push({
      key: 'confirmation', label: 'Signal confirmation gate', value: b.confirmation_mult ?? 1,
      effect: (b.confirmation_mult ?? 1) > 1 ? 'boost' : 'penalty', format: 'mult', max: 1.25,
    });
  }

  return factors;
}

function formatFactorValue(f: Factor): string {
  if (f.format === 'score') return `${Math.round(f.value * 100)}%`;
  if (f.format === 'mult') return `x${f.value.toFixed(2)}`;
  return `+${(f.value * 100).toFixed(0)}%`;
}

function getBarWidth(f: Factor): number {
  if (f.format === 'mult') {
    // For multipliers, center at 1.0: <1.0 = penalty, >1.0 = boost
    return Math.min(Math.abs(f.value - 1.0) / 0.3 * 100, 100);
  }
  return Math.min((f.value / f.max) * 100, 100);
}

const EFFECT_COLORS = {
  boost: { bar: 'bg-green-500/60', text: 'text-green-400', label: 'text-green-300' },
  penalty: { bar: 'bg-amber-500/60', text: 'text-amber-400', label: 'text-amber-300' },
  neutral: { bar: 'bg-gray-500/40', text: 'text-gray-400', label: 'text-gray-300' },
};

export const ScoreBreakdownDrawer = memo(function ScoreBreakdownDrawer({
  breakdown,
  finalScore,
  itemId,
  onClose,
  compareBreakdown,
  compareScore,
  compareTitle,
}: ScoreBreakdownDrawerProps) {
  const { t } = useTranslation();
  const factors = extractFactors(breakdown);
  const compareFactors = compareBreakdown ? extractFactors(compareBreakdown) : null;

  // Group factors
  const boosts = factors.filter(f => f.effect === 'boost');
  const penalties = factors.filter(f => f.effect === 'penalty');
  const neutrals = factors.filter(f => f.effect === 'neutral');

  // Signal gate status
  const signalCount = breakdown.signal_count ?? 0;
  const confirmedSignals = breakdown.confirmed_signals ?? [];

  return (
    <div className="border-t border-border bg-bg-primary/95 backdrop-blur-sm">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-border/50">
        <div className="flex items-center gap-3">
          <span className="text-lg font-bold font-mono text-white">
            {Math.round(finalScore * 100)}%
          </span>
          <span className="text-[10px] text-gray-500 uppercase tracking-wider">
            {t('scoreDrawer.title', 'Score Breakdown')}
          </span>
        </div>
        <button
          onClick={onClose}
          className="text-gray-500 hover:text-white transition-colors text-sm px-2 py-1"
          aria-label="Close score breakdown"
        >
          &times;
        </button>
      </div>

      <div className="px-4 py-3 space-y-4 max-h-[50vh] overflow-y-auto">
        {/* Confirmation Gate */}
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[10px] text-gray-500 uppercase tracking-wider">
            {t('scoreDrawer.signals', 'Signals')}
          </span>
          {['context', 'interest', 'ace', 'learned', 'dependency'].map(axis => {
            const confirmed = confirmedSignals.includes(axis);
            return (
              <span
                key={axis}
                className={`text-[10px] px-1.5 py-0.5 rounded border ${
                  confirmed
                    ? 'bg-green-500/15 text-green-400 border-green-500/30'
                    : 'bg-bg-tertiary text-gray-600 border-border'
                }`}
              >
                {confirmed ? '\u2713' : '\u2717'} {axis}
              </span>
            );
          })}
          <span className="text-[10px] text-gray-500 ml-1">
            {signalCount}/5
          </span>
        </div>

        {/* Boost factors */}
        {boosts.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.whyMatched', 'Why it matched')}
            factors={boosts}
            comparisons={compareFactors}
            itemId={itemId}
          />
        )}

        {/* Penalty factors */}
        {penalties.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.whatReduced', 'What reduced it')}
            factors={penalties}
            comparisons={compareFactors}
            itemId={itemId}
          />
        )}

        {/* Neutral factors */}
        {neutrals.length > 0 && (
          <FactorGroup
            label={t('scoreDrawer.neutral', 'Neutral')}
            factors={neutrals}
            comparisons={compareFactors}
            itemId={itemId}
          />
        )}

        {/* Comparison header */}
        {compareBreakdown && compareScore != null && (
          <div className="pt-2 border-t border-border/50">
            <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-1">
              {t('scoreDrawer.comparing', 'Comparing with')}
            </p>
            <p className="text-xs text-gray-400 truncate">{compareTitle}</p>
            <p className="text-sm font-mono text-white mt-1">
              {Math.round(compareScore * 100)}% vs {Math.round(finalScore * 100)}%
            </p>
          </div>
        )}
      </div>
    </div>
  );
});

// ============================================================================
// Factor Group — renders a labeled section of factor bars
// ============================================================================

function FactorGroup({
  label, factors, comparisons, itemId,
}: {
  label: string;
  factors: Factor[];
  comparisons: Factor[] | null;
  itemId: number;
}) {
  return (
    <div>
      <p className="text-[10px] text-gray-500 uppercase tracking-wider mb-1.5">{label}</p>
      <div className="space-y-1.5">
        {factors.map(f => (
          <FactorBar
            key={f.key}
            factor={f}
            compareValue={comparisons?.find(c => c.key === f.key)?.value}
            itemId={itemId}
          />
        ))}
      </div>
    </div>
  );
}

// ============================================================================
// Factor Bar — single scoring factor with bar, value, and teach-me feedback
// ============================================================================

function FactorBar({ factor, compareValue, itemId }: {
  factor: Factor;
  compareValue?: number;
  itemId: number;
}) {
  const [feedbackGiven, setFeedbackGiven] = useState<'up' | 'down' | null>(null);
  const colors = EFFECT_COLORS[factor.effect];
  const barWidth = getBarWidth(factor);

  const handleFeedback = useCallback(async (vote: 'up' | 'down') => {
    setFeedbackGiven(vote);
    try {
      await invoke('ace_record_interaction', {
        itemId,
        actionType: vote === 'up' ? 'click' : 'dismiss',
        actionData: { factor: factor.key, dwell_time_seconds: 0 },
        itemTopics: [factor.key],
        itemSource: 'score_feedback',
      });
    } catch {
      // Feedback is best-effort
    }
  }, [itemId, factor.key]);

  return (
    <div className="group flex items-center gap-2">
      {/* Label */}
      <span className={`text-[11px] w-28 flex-shrink-0 ${colors.label}`}>
        {factor.label}
      </span>

      {/* Bar container */}
      <div className="flex-1 h-4 bg-bg-tertiary rounded overflow-hidden relative">
        <div
          className={`h-full rounded transition-all duration-300 ${colors.bar}`}
          style={{ width: `${barWidth}%` }}
        />
        {/* Compare overlay */}
        {compareValue != null && (
          <div
            className="absolute top-0 h-full border-r-2 border-white/40"
            style={{ left: `${getBarWidth({ ...factor, value: compareValue })}%` }}
          />
        )}
      </div>

      {/* Value */}
      <span className={`text-[11px] font-mono w-12 text-right flex-shrink-0 ${colors.text}`}>
        {formatFactorValue(factor)}
      </span>

      {/* Teach Me — thumbs up/down (only visible on hover) */}
      <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
        {feedbackGiven ? (
          <span className="text-[10px] text-gray-600 w-8 text-center">
            {feedbackGiven === 'up' ? '\u2713' : '\u2717'}
          </span>
        ) : (
          <>
            <button
              onClick={() => handleFeedback('up')}
              className="text-[10px] text-gray-600 hover:text-green-400 transition-colors px-0.5"
              title="This factor was relevant"
              aria-label={`${factor.label} was relevant`}
            >
              +
            </button>
            <button
              onClick={() => handleFeedback('down')}
              className="text-[10px] text-gray-600 hover:text-amber-400 transition-colors px-0.5"
              title="This factor wasn't relevant"
              aria-label={`${factor.label} was not relevant`}
            >
              &minus;
            </button>
          </>
        )}
      </div>

      {/* Detail text */}
      {factor.detail && (
        <span className="text-[9px] text-gray-600 truncate max-w-[100px] flex-shrink-0" title={factor.detail}>
          {factor.detail}
        </span>
      )}
    </div>
  );
}

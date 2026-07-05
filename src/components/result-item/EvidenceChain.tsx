// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ExplanationFactor, FactorKind } from '../../types';

/**
 * Full ranked evidence chain for the expanded card.
 *
 * Renders the SAME chain the collapsed view reads (subtitle = factor 1,
 * chips = factors 2..4) — consistency by construction. Each row shows the
 * factor's named display, its concrete evidence, and a weight bar sized by
 * the factor's actual share of the score contribution.
 */

const KIND_COLORS: Record<FactorKind, string> = {
  SecurityAdvisory: 'bg-red-400',
  DependencyMatch: 'bg-emerald-400',
  ContextMatch: 'bg-accent-gold',
  InterestMatch: 'bg-blue-400',
  TopicMatch: 'bg-zinc-400',
  DecisionWindow: 'bg-purple-400',
  SkillGap: 'bg-cyan-400',
  LearnedPreference: 'bg-amber-400',
  CommunitySignal: 'bg-teal-400',
};

export const EvidenceChain = memo(function EvidenceChain({
  factors,
}: {
  factors: ExplanationFactor[];
}) {
  const { t } = useTranslation();
  if (factors.length === 0) return null;

  return (
    <div className="mb-3 p-2 bg-bg-primary/50 rounded border border-border/40">
      <div className="text-xs text-text-secondary font-medium mb-1.5">
        {t('results.evidenceChain')}
      </div>
      <ul className="space-y-1.5">
        {factors.map((f, i) => (
          <li key={`${f.kind}-${i}`} className="flex items-start gap-2">
            {/* Weight bar — width is the factor's share of the explained total */}
            <span className="flex-shrink-0 w-14 h-1.5 mt-1.5 bg-bg-tertiary rounded overflow-hidden">
              <span
                className={`block h-full rounded ${KIND_COLORS[f.kind] ?? 'bg-zinc-400'}`}
                style={{ width: `${Math.round(Math.min(Math.max(f.weight_share, 0), 1) * 100)}%` }}
              />
            </span>
            <span className="min-w-0">
              <span className="text-xs text-text-primary leading-snug block">
                {f.display}
              </span>
              <span className="text-[10px] text-text-muted leading-snug block">
                {f.evidence}
              </span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
});

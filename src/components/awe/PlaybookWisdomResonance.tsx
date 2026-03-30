// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

// ============================================================================
// Types
// ============================================================================

interface Props {
  moduleTopics: string[];
}

// ============================================================================
// Constants
// ============================================================================

const MASTERY_PREDICTION: Record<string, string> = {
  cold_start: 'Building foundation',
  accumulating: 'Patterns emerging',
  compounding: 'Accelerating mastery',
  mature: 'Deep understanding',
};

// ============================================================================
// Main Component
// ============================================================================

/**
 * PlaybookWisdomResonance — shows which playbook lessons align with real decision data.
 *
 * Renders a subtle gold-bordered section when the user's growth phase indicates
 * enough decisions to be meaningful. Returns null during cold start.
 */
export const PlaybookWisdomResonance = memo(function PlaybookWisdomResonance({ moduleTopics }: Props) {
  const { t } = useTranslation();
  const aweGrowthTrajectory = useAppStore(s => s.aweGrowthTrajectory);
  const loadAweGrowthTrajectory = useAppStore(s => s.loadAweGrowthTrajectory);

  useEffect(() => {
    void loadAweGrowthTrajectory();
  }, [loadAweGrowthTrajectory]);

  // Don't render during cold start or when no data
  if (!aweGrowthTrajectory) return null;
  if (aweGrowthTrajectory.growth_phase === 'cold_start') return null;

  const { growth_phase, decisions } = aweGrowthTrajectory;
  const mastery = MASTERY_PREDICTION[growth_phase] ?? 'Building foundation';

  // Count how many module topics align with the decision count
  // (simple heuristic: if you have decisions, your modules are reinforced)
  const alignedCount = Math.min(moduleTopics.length, Math.floor(decisions / 3));

  if (alignedCount === 0) return null;

  return (
    <div className="border border-accent-gold/15 rounded-lg p-3 mt-3 space-y-2">
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="text-accent-gold/60 text-xs">{'\u25C7'}</span>
        <h5 className="text-[10px] text-accent-gold/80 uppercase tracking-wider font-medium">
          {t('awe.playbook.title')}
        </h5>
      </div>

      {/* Resonance summary */}
      <p className="text-xs text-text-secondary leading-relaxed">
        {t('awe.playbook.reinforcement', { count: decisions, aligned: alignedCount })}
      </p>

      {/* Aligned topics */}
      <div className="flex flex-wrap gap-1.5">
        {moduleTopics.slice(0, alignedCount).map(topic => (
          <span
            key={topic}
            className="text-[10px] px-2 py-0.5 rounded-full bg-accent-gold/10 text-accent-gold/70 border border-accent-gold/15"
          >
            {topic}
          </span>
        ))}
      </div>

      {/* Mastery prediction */}
      <p className="text-[10px] text-text-muted">
        {t('awe.playbook.mastery')}: <span className="text-text-secondary">{mastery}</span>
      </p>
    </div>
  );
});

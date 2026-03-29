import { memo } from 'react';
import type { CompoundAdvantageScore } from '../../types/autophagy';
import type { KnowledgeGap } from '../../types/innovation';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { MomentumGauges } from './MomentumGauges';

// ---------------------------------------------------------------------------
// Component — data-dense gauge row as the hero
// ---------------------------------------------------------------------------

export interface MomentumHeroProps {
  advantage: CompoundAdvantageScore | null;
  history: number[];
  entries: RadarEntry[];
  gaps: KnowledgeGap[];
}

export const MomentumHero = memo(function MomentumHero({
  advantage, history, entries, gaps,
}: MomentumHeroProps) {
  return (
    <MomentumGauges
      advantage={advantage}
      history={history}
      entries={entries}
      gaps={gaps}
    />
  );
});

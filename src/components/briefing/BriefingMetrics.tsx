import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EngagementPulse } from '../EngagementPulse';
import { IntelligencePulse } from '../IntelligencePulse';
import { ScoringDelta } from '../ScoringDelta';
import { CompoundAdvantageScore } from '../CompoundAdvantageScore';
import type { IntelligencePulseData } from '../../types/autophagy';

interface BriefingMetricsProps {
  pulse: IntelligencePulseData | null;
}

export const BriefingMetrics = memo(function BriefingMetrics({ pulse }: BriefingMetricsProps) {
  const { t } = useTranslation();
  const [metricsExpanded, setMetricsExpanded] = useState(false);

  return (
    <div>
      <button
        onClick={() => setMetricsExpanded(prev => !prev)}
        aria-expanded={metricsExpanded}
        aria-label={t('briefing.intelligenceMetrics')}
        className="flex items-center gap-2 text-xs text-text-muted cursor-pointer py-2 w-full text-left"
      >
        <span>{t('briefing.intelligenceMetrics')}</span>
        <span className="text-[10px] bg-white/5 px-1.5 py-0.5 rounded">
          {pulse?.calibration_accuracy != null ? `${(pulse.calibration_accuracy * 100).toFixed(0)}% accuracy` : '\u2014'}
        </span>
        <span className={`ml-auto text-[10px] transition-transform duration-200 ${metricsExpanded ? 'rotate-90' : ''}`} aria-hidden="true">{'\u25B8'}</span>
      </button>
      {metricsExpanded && (
        <div className="space-y-3 pt-2">
          <EngagementPulse />
          <IntelligencePulse />
          <ScoringDelta />
          <CompoundAdvantageScore />
        </div>
      )}
    </div>
  );
});

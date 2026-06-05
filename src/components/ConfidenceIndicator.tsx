// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

interface ConfidenceIndicatorProps {
  confidence?: number;
  signalCount?: number;
  confirmedSignals?: string[];
}

export const ConfidenceIndicator = ({
  confidence,
  signalCount,
  confirmedSignals,
}: ConfidenceIndicatorProps) => {
  const { t } = useTranslation();

  if (signalCount != null) {
    const className =
      signalCount >= 4
        ? 'confidence-high'
        : signalCount >= 2
          ? 'confidence-medium'
          : 'confidence-low';
    const tooltip = confirmedSignals?.length
      ? confirmedSignals.join(', ')
      : undefined;
    return (
      <span
        className={`confidence-indicator ${className} text-xs text-text-muted ms-1 opacity-70`}
        title={tooltip}
      >
        {t('score.signalConcordance', {
          count: signalCount,
          total: 5,
        })}
      </span>
    );
  }

  if (!confidence) return null;

  // A qualitative tier, not a fabricated "±N%" margin of error. The model emits
  // a single self-reported confidence scalar — rendering `±(1-conf)%` dressed it
  // up as a statistical interval the system never computed (banned fabricated
  // precision). High / Medium / Low is what the scalar actually supports, and it
  // localizes cleanly.
  const formatConfidence = (conf: number) => {
    if (conf >= 0.8) {
      return { text: t('results.highConfidence'), className: 'confidence-high' };
    }
    if (conf >= 0.5) {
      return { text: t('results.mediumConfidence'), className: 'confidence-medium' };
    }
    return { text: t('results.lowConfidence'), className: 'confidence-low' };
  };

  const { text, className } = formatConfidence(confidence);

  return (
    <span className={`confidence-indicator ${className} text-xs text-text-muted ms-1 opacity-70`}>
      {text}
    </span>
  );
};

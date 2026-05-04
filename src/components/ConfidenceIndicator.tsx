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
          defaultValue: `${signalCount}/5`,
        })}
      </span>
    );
  }

  if (!confidence) return null;

  const formatConfidence = (conf: number) => {
    if (conf >= 0.8) {
      const margin = ((1 - conf) * 100).toFixed(0);
      return { text: `±${margin}%`, className: 'confidence-high' };
    }
    if (conf >= 0.5) {
      const margin = ((1 - conf) * 100).toFixed(0);
      return { text: `±${margin}%`, className: 'confidence-medium' };
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

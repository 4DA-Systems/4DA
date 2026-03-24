import { useTranslation } from 'react-i18next';

interface ConfidenceIndicatorProps {
  confidence?: number;
}

export const ConfidenceIndicator = ({ confidence }: ConfidenceIndicatorProps) => {
  const { t } = useTranslation();
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

interface ConfidenceIndicatorProps {
  confidence?: number;
}

export const ConfidenceIndicator = ({ confidence }: ConfidenceIndicatorProps) => {
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
    return { text: '⚠️ Low confidence', className: 'confidence-low' };
  };

  const { text, className } = formatConfidence(confidence);

  return (
    <span className={`confidence-indicator ${className} text-xs text-text-muted ml-1 opacity-70`}>
      {text}
    </span>
  );
};

import { memo } from 'react';

interface TasteCardData {
  id: number;
  title: string;
  snippet: string;
  sourceHint: string;
  categoryHint: string;
}

interface TasteTestCardProps {
  card: TasteCardData;
  onInterested: () => void;
  onSkip: () => void;
  onStrongInterest: () => void;
  isAnimating: boolean;
}

export const TasteTestCard = memo(function TasteTestCard({
  card,
  onInterested,
  onSkip,
  onStrongInterest,
  isAnimating,
}: TasteTestCardProps) {
  return (
    <div
      className={`bg-bg-secondary border border-border rounded-lg p-6 transition-all duration-200 ${
        isAnimating ? 'opacity-0 translate-x-4' : 'opacity-100 translate-x-0'
      }`}
    >
      {/* Header badges */}
      <div className="flex items-center justify-between mb-4">
        <span className="text-[11px] text-text-muted bg-bg-tertiary px-2 py-0.5 rounded">
          {card.categoryHint}
        </span>
        <span className="text-[11px] text-text-muted">
          {card.sourceHint}
        </span>
      </div>

      {/* Content */}
      <h3 className="text-white font-medium text-base mb-3 leading-snug">
        {card.title}
      </h3>
      <p className="text-text-secondary text-sm leading-relaxed mb-6 line-clamp-3">
        {card.snippet}
      </p>

      {/* Actions */}
      <div className="flex items-center gap-3">
        <button
          onClick={onInterested}
          className="flex-1 bg-white text-black font-medium text-sm py-2.5 px-4 rounded-md hover:bg-gray-100 transition-colors"
        >
          I'd read this
        </button>
        <button
          onClick={onSkip}
          className="flex-1 border border-border text-text-secondary text-sm py-2.5 px-4 rounded-md hover:bg-bg-tertiary transition-colors"
        >
          Skip
        </button>
        <button
          onClick={onStrongInterest}
          className="w-10 h-10 flex items-center justify-center border border-border rounded-md hover:bg-bg-tertiary hover:border-[#D4AF37] hover:text-[#D4AF37] text-text-muted transition-colors"
          title="Love this"
        >
          ★
        </button>
      </div>
    </div>
  );
});

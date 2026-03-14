import { useTranslation } from 'react-i18next';

interface StreetsSuggestion {
  module_id: string;
  module_title: string;
  reason: string;
  match_strength: number;
}

interface Props {
  suggestion: StreetsSuggestion;
  onOpen: (moduleId: string) => void;
  onDismiss: (moduleId: string) => void;
}

export function StreetsContextCard({ suggestion, onOpen, onDismiss }: Props) {
  const { t } = useTranslation();

  return (
    <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-5 space-y-3">
      <div className="flex items-center gap-2">
        <span className="px-2 py-0.5 bg-[#D4AF37]/15 text-[#D4AF37] text-[10px] font-bold rounded uppercase tracking-wider">
          {t('streets:streets.title')}
        </span>
        <h3 className="text-sm font-medium text-[#D4AF37]">
          {suggestion.module_title}
        </h3>
      </div>

      <p className="text-xs text-[#A0A0A0] leading-relaxed">
        {suggestion.reason}
      </p>

      {/* Match strength indicator */}
      <div className="flex items-center gap-2">
        <div className="flex-1 h-1 bg-[#1F1F1F] rounded-full overflow-hidden">
          <div
            className="h-full bg-[#D4AF37]/60 rounded-full transition-all"
            style={{ width: `${Math.round(suggestion.match_strength * 100)}%` }}
          />
        </div>
        <span className="text-[10px] text-[#8A8A8A]">
          {t('briefing.streetsMatchStrength', { pct: Math.round(suggestion.match_strength * 100) })}
        </span>
      </div>

      <div className="flex items-center gap-3 pt-1">
        <button
          onClick={() => onOpen(suggestion.module_id)}
          className="px-4 py-1.5 text-xs font-medium bg-[#D4AF37] text-black rounded-lg hover:bg-[#C4A030] transition-colors"
        >
          {t('briefing.streetsOpenModule', { id: suggestion.module_id })}
        </button>
        <button
          onClick={() => onDismiss(suggestion.module_id)}
          className="text-xs text-[#8A8A8A] hover:text-[#A0A0A0] transition-colors"
        >
          {t('briefing.streetsNotNow')}
        </button>
      </div>
    </div>
  );
}

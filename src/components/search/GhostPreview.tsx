import { useTranslation } from 'react-i18next';

export interface GhostPreviewData {
  total_results: number;
  hidden_results: number;
  decision_count: number;
  gap_count: number;
  synthesis_available: boolean;
}

interface GhostPreviewProps {
  preview: GhostPreviewData;
}

export function GhostPreview({ preview }: GhostPreviewProps) {
  const { t } = useTranslation();

  const lines: { icon: string; label: string; accent: string; show: boolean }[] = [
    { icon: '\u2726', label: t('search.ghostSynthesis'), accent: 'text-cyan-400/60', show: preview.synthesis_available },
    { icon: '\u2192', label: t('search.ghostMoreResults', { count: preview.hidden_results }), accent: 'text-gray-400', show: preview.hidden_results > 0 },
    { icon: '\u2696', label: t('search.ghostDecisions', { count: preview.decision_count }), accent: 'text-amber-400/60', show: preview.decision_count > 0 },
    { icon: '\u25CE', label: t('search.ghostGaps', { count: preview.gap_count }), accent: 'text-red-400/60', show: preview.gap_count > 0 },
  ];

  const visibleLines = lines.filter((l) => l.show);
  if (visibleLines.length === 0) return null;

  return (
    <div className="rounded-lg bg-[#D4AF37]/[0.03] p-3 border border-[#D4AF37]/10">
      <div className="text-[10px] text-[#D4AF37]/70 uppercase tracking-wider font-medium mb-2">
        {t('search.proIntelligence')}
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-1">
        {visibleLines.map((line, i) => (
          <span key={i} className="flex items-center gap-1.5 text-xs text-gray-400">
            <span className={line.accent}>{line.icon}</span>
            {line.label}
          </span>
        ))}
      </div>
    </div>
  );
}

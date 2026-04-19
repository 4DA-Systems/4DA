// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { trackEvent } from '../../hooks/use-telemetry';

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
    { icon: '\u2192', label: t('search.ghostMoreResults', { count: preview.hidden_results }), accent: 'text-text-secondary', show: preview.hidden_results > 0 },
    { icon: '\u2696', label: t('search.ghostDecisions', { count: preview.decision_count }), accent: 'text-amber-400/60', show: preview.decision_count > 0 },
    { icon: '\u25CE', label: t('search.ghostGaps', { count: preview.gap_count }), accent: 'text-red-400/60', show: preview.gap_count > 0 },
  ];

  const visibleLines = lines.filter((l) => l.show);

  // Track ghost preview impression on render
  useEffect(() => {
    if (visibleLines.length > 0) trackEvent('ghost_preview_shown');
  }, [visibleLines.length]);

  if (visibleLines.length === 0) return null;

  return (
    <div className="rounded-lg bg-accent-gold/[0.03] p-3 border border-accent-gold/10 cursor-pointer" role="button" tabIndex={0} aria-label={t('search.proIntelligence')} onClick={() => trackEvent('ghost_preview_clicked')} onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); trackEvent('ghost_preview_clicked'); } }}>
      <div className="text-[10px] text-accent-gold/70 uppercase tracking-wider font-medium mb-2">
        {t('search.proIntelligence')}
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-1">
        {visibleLines.map((line, i) => (
          <span key={i} className="flex items-center gap-1.5 text-xs text-text-secondary">
            <span className={line.accent}>{line.icon}</span>
            {line.label}
          </span>
        ))}
      </div>
    </div>
  );
}

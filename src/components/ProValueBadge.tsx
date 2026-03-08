import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

/**
 * Compact Pro value summary shown in the app header.
 * Displays key metrics in a single line: signals, gaps, hours saved.
 * NOT Pro-gated — free users see what they're missing.
 * Reads from Zustand store (loaded once on mount via loadProValueReport).
 */
export const ProValueBadge = memo(function ProValueBadge() {
  const { t } = useTranslation();
  const report = useAppStore((s) => s.proValueReport);

  if (!report) return null;

  const { signals_detected, knowledge_gaps_caught, estimated_hours_saved } = report;

  // Don't show if there's nothing to report
  if (signals_detected === 0 && knowledge_gaps_caught === 0 && estimated_hours_saved === 0) {
    return null;
  }

  const parts: string[] = [];
  if (signals_detected > 0) parts.push(t('pro.signals', { count: signals_detected }));
  if (knowledge_gaps_caught > 0) parts.push(t('pro.gaps', { count: knowledge_gaps_caught }));
  if (estimated_hours_saved > 0) parts.push(t('pro.hoursSaved', { hours: estimated_hours_saved.toFixed(1) }));

  if (parts.length === 0) return null;

  return (
    <div
      className="hidden md:flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-bg-tertiary/50 border border-border/50"
      title={t('pro.intelligenceSummary', { summary: parts.join(', '), days: report.period_days })}
    >
      <svg width="12" height="12" viewBox="0 0 16 16" fill="none" className="text-text-muted flex-shrink-0">
        <rect x="1" y="8" width="3" height="6" rx="0.5" fill="currentColor" opacity="0.4" />
        <rect x="6" y="4" width="3" height="10" rx="0.5" fill="currentColor" opacity="0.6" />
        <rect x="11" y="1" width="3" height="13" rx="0.5" fill="currentColor" opacity="0.9" />
      </svg>
      <span className="text-[10px] text-text-muted whitespace-nowrap">
        {parts.join(' · ')}
      </span>
    </div>
  );
});

import { useState, useEffect, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ProValueReport } from '../types';

/**
 * Compact Pro value summary shown in the app header.
 * Displays key metrics in a single line: signals, gaps, hours saved.
 * NOT Pro-gated — free users see what they're missing.
 */
export const ProValueBadge = memo(function ProValueBadge() {
  const [report, setReport] = useState<ProValueReport | null>(null);

  useEffect(() => {
    invoke<ProValueReport>('get_pro_value_report')
      .then(setReport)
      .catch(() => {/* silently ignore */});
  }, []);

  if (!report) return null;

  const { signals_detected, knowledge_gaps_caught, estimated_hours_saved } = report;

  // Don't show if there's nothing to report
  if (signals_detected === 0 && knowledge_gaps_caught === 0 && estimated_hours_saved === 0) {
    return null;
  }

  const parts: string[] = [];
  if (signals_detected > 0) parts.push(`${signals_detected} signal${signals_detected !== 1 ? 's' : ''}`);
  if (knowledge_gaps_caught > 0) parts.push(`${knowledge_gaps_caught} gap${knowledge_gaps_caught !== 1 ? 's' : ''}`);
  if (estimated_hours_saved > 0) parts.push(`~${estimated_hours_saved.toFixed(1)}h saved`);

  if (parts.length === 0) return null;

  return (
    <div
      className="hidden md:flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-bg-tertiary/50 border border-border/50"
      title={`Pro Intelligence: ${parts.join(', ')} (last ${report.period_days} days)`}
    >
      <svg width="12" height="12" viewBox="0 0 16 16" fill="none" className="text-gray-500 flex-shrink-0">
        <rect x="1" y="8" width="3" height="6" rx="0.5" fill="currentColor" opacity="0.4" />
        <rect x="6" y="4" width="3" height="10" rx="0.5" fill="currentColor" opacity="0.6" />
        <rect x="11" y="1" width="3" height="13" rx="0.5" fill="currentColor" opacity="0.9" />
      </svg>
      <span className="text-[10px] text-gray-500 whitespace-nowrap">
        {parts.join(' · ')}
      </span>
    </div>
  );
});

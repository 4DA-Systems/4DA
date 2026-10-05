// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Presentation chrome for the theme map: loading / empty / error states and
// the one-line category legend. Split from ContentGraphView (size gate).
import { useTranslation } from 'react-i18next';

import { CategoryMark } from './graph-marks';

export function ErrorState({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: 'var(--color-bg-primary)' }}>
      <div className="flex flex-col items-center gap-2">
        <span style={{ color: 'var(--color-text-secondary)', fontSize: 14, fontFamily: 'Inter, sans-serif' }}>
          {t('signals.graphError', 'The graph could not be built')}
        </span>
        <button
          onClick={onRetry}
          className="px-3 py-1 text-xs rounded border transition-colors hover:bg-bg-tertiary"
          style={{ color: 'var(--color-text-primary)', borderColor: 'var(--color-border)' }}
        >
          {t('action.retry')}
        </button>
      </div>
    </div>
  );
}

export function LoadingState() {
  const { t } = useTranslation();
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: 'var(--color-bg-primary)' }}>
      <div className="flex flex-col items-center gap-3">
        <div className="w-8 h-8 border-2 border-text-primary/30 border-t-text-primary rounded-full animate-spin" />
        <span style={{ color: 'var(--color-text-secondary)', fontSize: 13, fontFamily: 'Inter, sans-serif' }}>
          {t('action.loading')}
        </span>
      </div>
    </div>
  );
}

export function EmptyState() {
  const { t } = useTranslation();
  return (
    <div className="h-full min-h-[500px] flex items-center justify-center" style={{ backgroundColor: 'var(--color-bg-primary)' }}>
      <div className="flex flex-col items-center gap-2">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" className="stroke-text-muted" strokeWidth="1.5">
          <circle cx="12" cy="12" r="3" />
          <circle cx="4" cy="8" r="2" />
          <circle cx="20" cy="8" r="2" />
          <circle cx="4" cy="16" r="2" />
          <circle cx="20" cy="16" r="2" />
          <line x1="9.5" y1="10.5" x2="5.5" y2="8.5" />
          <line x1="14.5" y1="10.5" x2="18.5" y2="8.5" />
          <line x1="9.5" y1="13.5" x2="5.5" y2="15.5" />
          <line x1="14.5" y1="13.5" x2="18.5" y2="15.5" />
        </svg>
        <span style={{ color: 'var(--color-text-muted)', fontSize: 14, fontFamily: 'Inter, sans-serif' }}>
          {t('signals.graphEmpty')}
        </span>
        <span style={{ color: 'var(--color-text-muted)', fontSize: 12, fontFamily: 'Inter, sans-serif' }}>
          {t('signals.graphEmptySub')}
        </span>
      </div>
    </div>
  );
}

interface GraphLegendProps {
  categories: readonly string[];
}

/** One line: the category silhouettes present in this map. Red marks
 *  security and gold marks your stack (graph-marks.tsx), so colour needs no
 *  key beyond the column it already heads. */
export function GraphLegend({ categories }: GraphLegendProps) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center flex-wrap gap-x-3 gap-y-1">
      {categories.map((cat) => (
        <span key={cat} className="inline-flex items-center gap-1.5 text-[11px]" style={{ color: 'var(--color-text-muted)' }}>
          <CategoryMark category={cat} surface="var(--color-bg-primary)" />
          {t(`signals.graphCat_${cat}`)}
        </span>
      ))}
    </div>
  );
}

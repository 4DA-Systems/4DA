// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Presentation chrome for the content graph: cluster labels, loading/empty
// states, and the category legend. Extracted from ContentGraphView so the
// view stays within size limits and owns only data flow + interactions.
import { useTranslation } from 'react-i18next';

import { CATEGORY_COLORS, CATEGORY_SHAPES } from './ContentGraphNode';

export function ClusterLabelNode({ data }: { data: { label: string; count: number } }) {
  return (
    <div
      style={{
        color: 'var(--color-text-secondary)',
        fontSize: 11,
        fontWeight: 600,
        fontFamily: 'Inter, sans-serif',
        letterSpacing: '0.03em',
        textTransform: 'uppercase',
        pointerEvents: 'none',
        whiteSpace: 'nowrap',
        // Halo in the page color lifts the label off edge lines in both themes
        textShadow: '0 1px 4px var(--color-bg-primary)',
        transform: 'translateX(-50%)',
      }}
    >
      {data.label}
      <span style={{ color: 'var(--color-text-muted)', fontWeight: 400, marginLeft: 4, fontSize: 10 }}>
        ({data.count})
      </span>
    </div>
  );
}

/// Soft disc behind each cluster's members: the theme grouping is visible at
/// fit zoom instead of only inferable from proximity (the "starfield" gap —
/// live audit 2026-07-19). Non-interactive by construction.
export function ClusterHullNode({ data }: { data: { radius: number } }) {
  const d = data.radius * 2;
  return (
    <div
      style={{
        width: d,
        height: d,
        borderRadius: '50%',
        border: '1px dashed var(--color-border)',
        backgroundColor: 'color-mix(in srgb, var(--color-text-primary) 3%, transparent)',
        pointerEvents: 'none',
      }}
    />
  );
}

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
  categories: string[];
  anyAffects: boolean;
  /** Edge types present in the current graph — line semantics were
   *  previously hover-only (audit 2026-07-19). */
  edgeTypes: string[];
}

// Mirrors EDGE_STYLES in ContentGraphEdge (the swatch IS the line style).
const EDGE_LEGEND: Record<string, { color: string; dasharray?: string; width: number; labelKey: string; fallback: string }> = {
  semantic: { color: '#6366F1', width: 1.5, labelKey: 'signals.graphEdgeSemantic', fallback: 'related content' },
  chain: { color: '#F59E0B', dasharray: '4 2', width: 1.5, labelKey: 'signals.graphEdgeChain', fallback: 'signal chain (your stack)' },
  convergence: { color: '#22C55E', width: 2.5, labelKey: 'signals.graphEdgeConvergence', fallback: 'both' },
};
const EDGE_LEGEND_ORDER = ['semantic', 'chain', 'convergence'] as const;

export function GraphLegend({ categories, anyAffects, edgeTypes }: GraphLegendProps) {
  const { t } = useTranslation();
  return (
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: '4px 12px',
        maxWidth: 300,
        padding: '8px 10px',
        backgroundColor: 'var(--color-bg-secondary)',
        border: '1px solid var(--color-border)',
        borderRadius: 8,
        fontFamily: 'Inter, sans-serif',
      }}
    >
      {categories.map((cat) => {
        const shape = CATEGORY_SHAPES[cat];
        return (
          <span
            key={cat}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 5,
              fontSize: 10,
              color: 'var(--color-text-secondary)',
              whiteSpace: 'nowrap',
            }}
          >
            <span
              style={{
                width: 9,
                height: 9,
                borderRadius: shape?.borderRadius ?? '50%',
                transform: shape?.rotate ? 'rotate(45deg)' : undefined,
                backgroundColor: CATEGORY_COLORS[cat],
                position: 'relative',
                display: 'inline-block',
              }}
            >
              {shape?.donut && (
                <span
                  style={{
                    position: 'absolute',
                    inset: '30%',
                    borderRadius: '50%',
                    backgroundColor: 'var(--color-bg-secondary)',
                  }}
                />
              )}
            </span>
            {t(`signals.graphCat_${cat}`)}
          </span>
        );
      })}
      {anyAffects && (
        <span
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 5,
            fontSize: 10,
            color: 'var(--color-text-secondary)',
            whiteSpace: 'nowrap',
          }}
        >
          {/* Mirrors the node treatment: max-contrast core + gold ring. */}
          <span
            style={{
              width: 9,
              height: 9,
              borderRadius: '50%',
              backgroundColor: 'var(--color-text-primary)',
              border: '2px solid var(--color-accent-gold)',
              display: 'inline-block',
            }}
          />
          {t('signals.graphAffectsYou')}
        </span>
      )}
      {EDGE_LEGEND_ORDER.filter((k) => edgeTypes.includes(k)).map((k) => {
        const s = EDGE_LEGEND[k]!;
        return (
          <span
            key={k}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 5,
              fontSize: 10,
              color: 'var(--color-text-secondary)',
              whiteSpace: 'nowrap',
            }}
          >
            <svg width="18" height="6" aria-hidden="true">
              <line
                x1="0"
                y1="3"
                x2="18"
                y2="3"
                stroke={s.color}
                strokeWidth={s.width}
                strokeDasharray={s.dasharray}
              />
            </svg>
            {t(s.labelKey, s.fallback)}
          </span>
        );
      })}
    </div>
  );
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Status strip under the content graph: coverage notes and the window toggle.
// Split from ContentGraphView (size gate).
import { useTranslation } from 'react-i18next';

import type { ContentGraph } from '../../types/graph';

const TIME_WINDOWS = [7, 14, 30] as const;

interface Props {
  meta: ContentGraph['meta'] | null;
  days: number;
  onDaysChange: (days: number) => void;
}

export default function ContentGraphFooter({ meta, days, onDaysChange }: Props) {
  const { t } = useTranslation();
  return (
    <div
      className="flex items-center justify-between px-4 py-2 border-t"
      style={{ backgroundColor: 'var(--color-bg-secondary)', borderColor: 'var(--color-border)' }}
    >
      <div className="flex gap-4 text-[11px]" style={{ color: 'var(--color-text-muted)', fontFamily: 'JetBrains Mono, monospace' }}>
        {meta && (
          <>
            <span>{meta.total_items} {t('signals.graphNodes', 'nodes')}</span>
            <span>{meta.total_edges} {t('signals.graphEdges', 'edges')}</span>
            <span>{meta.cluster_count} {t('signals.graphClusters', 'clusters')}</span>
            {meta.collapsed_items > 0 && (
              <span>{t('signals.graphCollapsedNote', { items: meta.collapsed_items, stories: meta.story_count })}</span>
            )}
            {/* Honest coverage: the map is the top slice of the window, not
                the window. The old "+2 in List only" line counted just the
                cap overflow while thousands sat below the load cutoff. */}
            {meta.window_candidates > meta.total_items + meta.collapsed_items && (
              <span>{t('signals.graphCoverageNote', 'top {{shown}} of {{total}} this window', {
                shown: meta.total_items + meta.collapsed_items,
                total: meta.window_candidates,
              })}</span>
            )}
            {/* Corpus parity ramp (Phase 95): curated vs unjudged in ITEM
                units (P2.14 — story collapse can't inflate the ramp). */}
            {meta.curated_items > 0 && meta.curated_items < meta.total_items + meta.collapsed_items && (
              <span>{t('signals.graphCuratedNote', '{{curated}} curated · {{recent}} recent unjudged', {
                curated: meta.curated_items,
                recent: meta.total_items + meta.collapsed_items - meta.curated_items,
              })}</span>
            )}
          </>
        )}
      </div>
      {/* The 7/14/30d toggle renders only when the windows would actually
          differ (curated verdicts older than 7d exist) — a control that
          does nothing is a cold-start-doctrine violation. Kept visible if
          the user already switched off the default so they can get back. */}
      {(meta?.windows_differ || days !== 7) && (
      <div className="flex items-center gap-1">
        {TIME_WINDOWS.map((w) => (
          <button
            key={w}
            onClick={() => onDaysChange(w)}
            className={`px-2 py-0.5 text-[10px] rounded transition-colors ${
              days === w
                ? 'bg-bg-tertiary text-text-primary'
                : 'text-text-muted hover:text-text-secondary'
            }`}
            style={{ fontFamily: 'JetBrains Mono, monospace' }}
          >
            {/* eslint-disable-next-line i18next/no-literal-string */}
            {w}d
          </button>
        ))}
      </div>
      )}
    </div>
  );
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

/** Citation rows rendered before the "show more" control. */
export const EVIDENCE_COLLAPSE_THRESHOLD = 2;

function formatFreshness(
  days: number,
  t: (key: string, opts?: Record<string, unknown>) => string,
): string {
  const d = Math.round(days);
  if (d <= 0) return t('preemption.freshness.today');
  if (d === 1) return t('preemption.freshness.yesterday');
  if (d < 7) return t('preemption.freshness.daysAgo', { count: d });
  if (d < 30) return t('preemption.freshness.weeksAgo', { count: Math.floor(d / 7) });
  return t('preemption.freshness.monthsAgo', { count: Math.floor(d / 30) });
}

/**
 * The card's citation list. Renders `EVIDENCE_COLLAPSE_THRESHOLD` rows and
 * collapses the rest.
 *
 * AD-035: the LIST response embeds only the rows this renders collapsed, so
 * `hiddenExtra` (derived from `evidence_total`) carries the citations held
 * back server-side — counted in the header and the "show more" label, and
 * fetched by `onExpand` on the first expansion. A failed fetch still expands
 * the embedded rows.
 */
export const EvidenceList = memo(function EvidenceList({
  evidence,
  cardTitle,
  hiddenExtra,
  onExpand,
}: {
  evidence: EvidenceItem['evidence'];
  cardTitle?: string;
  hiddenExtra: number;
  onExpand?: () => Promise<void>;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const filtered = cardTitle
    ? evidence.filter(e => e.title.toLowerCase() !== cardTitle.toLowerCase())
    : evidence;

  if (filtered.length === 0 && hiddenExtra === 0) return null;

  const shown = expanded ? filtered : filtered.slice(0, EVIDENCE_COLLAPSE_THRESHOLD);
  const canCollapse = filtered.length > EVIDENCE_COLLAPSE_THRESHOLD || hiddenExtra > 0;

  return (
    <div className="mt-3 pt-3 border-t border-border/50">
      <h4 className="text-[10px] font-medium text-text-muted uppercase tracking-wider mb-2">
        {t('preemption.evidence')} ({evidence.length + hiddenExtra})
      </h4>
      <ul className="space-y-1.5">
        {shown.map((e, i) => (
          <li key={i} className="flex items-baseline gap-2 text-xs min-w-0">
            <span className="shrink-0 font-mono text-[10px] uppercase text-text-muted w-14 truncate">
              {e.source}
            </span>
            {e.url ? (
              <a
                href={e.url}
                target="_blank"
                rel="noopener noreferrer"
                className="flex-1 min-w-0 text-text-secondary hover:text-text-primary transition-colors truncate"
                title={e.title}
              >
                {e.title}
              </a>
            ) : (
              <span
                className="flex-1 min-w-0 text-text-secondary truncate"
                title={e.title}
              >
                {e.title}
              </span>
            )}
            <span className="shrink-0 text-[10px] text-text-muted tabular-nums">
              {formatFreshness(e.freshness_days, t)}
            </span>
          </li>
        ))}
      </ul>
      {canCollapse && (
        <button
          type="button"
          onClick={() => {
            if (expanded) {
              setExpanded(false);
              return;
            }
            // Lazy detail fetch (AD-035): the list shipped only the rendered
            // rows; the rest hydrate from get_preemption_item_detail. On a
            // fetch failure the embedded rows still expand.
            void (async () => {
              try { await onExpand?.(); } catch { /* keep embedded rows */ }
              setExpanded(true);
            })();
          }}
          className="mt-2 text-[11px] text-text-muted hover:text-text-secondary transition-colors"
        >
          {expanded
            ? t('preemption.evidence.showLess')
            : t('preemption.evidence.showMore', {
                count: Math.max(filtered.length - EVIDENCE_COLLAPSE_THRESHOLD, 0) + hiddenExtra,
              })}
        </button>
      )}
    </div>
  );
});

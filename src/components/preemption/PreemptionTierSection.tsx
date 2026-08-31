// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState } from 'react';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { ItemCard } from './PreemptionCard';

interface PreemptionTierSectionProps {
  dotColor: string;
  borderColor: string;
  title: string;
  subtitle: string;
  items: EvidenceItem[];
  surfacedRef: React.RefObject<Set<string>>;
  onDismiss: (id: string) => void;
  emptyText: string;
  /**
   * When set and `items.length` exceeds it, only the first `maxVisible` items
   * render until the user expands. Keeps a long RANKED list (the Upgrade Plan
   * can run to 100+ steps on a large stack) scannable — the top-ranked steps
   * that matter most stay visible, the rest are one click away. Nothing is
   * suppressed (the full set is still in the plan snapshot the CLI/MCP read).
   * The subtitle should reflect the TOTAL count, not the visible count.
   */
  maxVisible?: number;
  /**
   * Label for the expand control, given the hidden count (the parent translates,
   * e.g. `preemption.evidence.showMore`). Required for the cap to render.
   */
  showMoreLabel?: (hidden: number) => string;
  /**
   * Items the LIST transport held back server-side (AD-035): counted in the
   * expand control and the subtitle, fetched by `onExpand`. The section is
   * expandable whenever this is non-zero, even if the shipped array fits
   * under `maxVisible`.
   */
  hiddenExtra?: number;
  /**
   * Called before expanding when server-held items exist — refetches the
   * uncapped list (`fullPlan`), which flows back in through `items`.
   */
  onExpand?: () => void | Promise<void>;
}

export const PreemptionTierSection = memo(function PreemptionTierSection({
  dotColor,
  borderColor,
  title,
  subtitle,
  items,
  surfacedRef,
  onDismiss,
  emptyText,
  maxVisible,
  showMoreLabel,
  hiddenExtra = 0,
  onExpand,
}: PreemptionTierSectionProps) {
  const [expanded, setExpanded] = useState(false);
  const capped =
    maxVisible !== undefined && !expanded && (items.length > maxVisible || hiddenExtra > 0);
  const visibleItems = capped ? items.slice(0, maxVisible) : items;
  const hidden = items.length - visibleItems.length + (expanded ? 0 : hiddenExtra);

  return (
    <section className="mb-4" aria-label={title}>
      <div className="bg-bg-secondary rounded-lg border overflow-hidden" style={{ borderColor }}>
        <div className="px-4 py-3 border-b border-border flex items-center gap-2">
          <div className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: dotColor }} />
          <h3 className="text-sm font-medium text-text-primary flex-1">{title}</h3>
          <span className="text-xs text-[#8A8A8A]">{subtitle}</span>
        </div>
        {items.length > 0 ? (
          <div className="p-4 space-y-4">
            {visibleItems.map(item => (
              <ItemCard key={item.id} item={item} surfacedRef={surfacedRef} onDismiss={onDismiss} />
            ))}
            {capped && showMoreLabel && (
              <button
                type="button"
                onClick={() => {
                  if (!onExpand) {
                    setExpanded(true);
                    return;
                  }
                  // Server-held items (AD-035): fetch the uncapped list, then
                  // expand — the refetched steps arrive through `items`.
                  void (async () => {
                    try { await onExpand(); } catch { /* keep shipped items */ }
                    setExpanded(true);
                  })();
                }}
                className="w-full text-xs text-text-secondary hover:text-text-primary py-2 border-t border-border transition-colors"
              >
                {showMoreLabel(hidden)}
              </button>
            )}
          </div>
        ) : (
          <div className="px-4 py-4">
            <p className="text-xs text-[#8A8A8A]">{emptyText}</p>
          </div>
        )}
      </div>
    </section>
  );
});

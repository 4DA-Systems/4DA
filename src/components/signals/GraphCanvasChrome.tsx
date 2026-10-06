// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Canvas chrome for the Graph view: group headers, hull discs and the shelf
// header, as React Flow node types. Split from ContentGraphView (size gate).
import { useTranslation } from 'react-i18next';

import { zoomInvariant } from './graph-zoom';
import {
  HEADER_COUNT_FONT_PX,
  HEADER_FONT_PX,
  HEADER_LINE,
  LANE_HEADER_GAP_PX,
  LANE_HEADER_RULE_PX,
} from './content-graph-label-layout';

/** Group header in sentence case (the old letter-spaced capitals took ~40%
 *  more width and collided more). LabelCollisionLayer nudges it inside its
 *  hull (--cg-dx / --cg-dy) or suppresses it (data-cg-suppressed). The
 *  "Your stack" group reads in gold, like the Themes column. */
export function ClusterLabelNode({ id, data }: { id: string; data: { label: string; count: number; stack?: boolean } }) {
  return (
    <div
      data-cg-label-id={`cluster:${id}`}
      style={{
        color: data.stack ? 'var(--color-accent-gold)' : 'var(--color-text-primary)',
        fontSize: zoomInvariant(HEADER_FONT_PX),
        lineHeight: HEADER_LINE,
        fontWeight: 600,
        fontFamily: 'Inter, sans-serif',
        pointerEvents: 'none',
        whiteSpace: 'nowrap',
        textShadow: '0 1px 4px var(--color-bg-primary), 0 0 2px var(--color-bg-primary)',
        transform: 'translate(calc(-50% + var(--cg-dx, 0px)), var(--cg-dy, 0px))',
      }}
    >
      {data.label}
      <span style={{ color: 'var(--color-text-muted)', fontWeight: 400, marginLeft: 4, fontSize: zoomInvariant(HEADER_COUNT_FONT_PX) }}>
        {data.count}
      </span>
    </div>
  );
}

/** Header over the shelf of unthemed items. It hangs ABOVE its anchor (the
 *  shelf's first row) so its zoom-invariant text grows upward, never into
 *  the row's labels. */
export function LaneLabelNode({ data }: { data: { count: number } }) {
  const { t } = useTranslation();
  return (
    <div
      data-cg-label-id="lane"
      style={{
        color: 'var(--color-text-muted)',
        fontSize: zoomInvariant(HEADER_FONT_PX),
        lineHeight: HEADER_LINE,
        fontWeight: 600,
        fontFamily: 'Inter, sans-serif',
        pointerEvents: 'none',
        whiteSpace: 'nowrap',
        borderBottom: '1px dashed var(--color-border)',
        paddingBottom: zoomInvariant(LANE_HEADER_RULE_PX - 1),
        textShadow: '0 1px 4px var(--color-bg-primary)',
        transform: `translateY(calc(-100% - ${zoomInvariant(LANE_HEADER_GAP_PX)}))`,
      }}
    >
      {t('signals.graphLaneLabel', { count: data.count })}
    </div>
  );
}

/** Soft disc behind each group so grouping reads at fit zoom; gold-edged for
 *  the "Your stack" group. Non-interactive by construction. */
export function ClusterHullNode({ data }: { data: { radius: number; stack?: boolean } }) {
  const d = data.radius * 2;
  return (
    <div
      style={{
        width: d,
        height: d,
        borderRadius: '50%',
        border: data.stack
          ? '1px solid color-mix(in srgb, var(--color-accent-gold) 45%, transparent)'
          : '1px dashed var(--color-border)',
        backgroundColor: data.stack
          ? 'color-mix(in srgb, var(--color-accent-gold) 5%, transparent)'
          : 'color-mix(in srgb, var(--color-text-primary) 3%, transparent)',
        pointerEvents: 'none',
      }}
    />
  );
}

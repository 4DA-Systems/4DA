// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { Handle, Position, type NodeProps, type Node } from '@xyflow/react';
import { useTranslation } from 'react-i18next';
// Theme overrides for React Flow's chrome (zoom Controls, MiniMap). Imported
// here — the node module is always in the graph bundle — so it also applies to
// the Controls/MiniMap rendered by ContentGraphView without editing that file.
import './content-graph.css';
import { zoomInvariant } from './graph-zoom';
import {
  NODE_LABEL_FONT_PX,
  NODE_LABEL_LINE,
  NODE_LABEL_MAX_W,
  nodeLabelText,
  nodeMarkSize,
} from './content-graph-label-layout';

interface ContentNodeData {
  title: string;
  url: string | null;
  source_type: string;
  relevance_score: number;
  signal_type: string | null;
  signal_priority: string | null;
  primary_topic: string | null;
  cluster_id: string | null;
  /** Items this node represents; >1 = a story of collapsed near-duplicates. */
  member_count: number;
  /** Item ids of ALL members including the representative — detail panel hydration. */
  member_ids: number[];
  /** Content category — the primary color + shape channel. */
  category: string;
  /** Linked to the user's declared dependencies → gold ring. */
  affects_you: boolean;
  isNew?: boolean;
  [key: string]: unknown;
}

export type ContentNode = Node<ContentNodeData, 'contentNode'>;

// Category is the color channel (source identity lives in the tooltip — 22
// sources shared 8 hues, five of them one red; undecodable by construction).
// Palette validated with the dataviz six-checks validator on the #0A0A0A
// surface: CVD-adjacent ΔE 9.2 (deutan), normal-vision 21.6, contrast ≥3:1.
// Each category ALSO carries a distinct silhouette (shape), so identity never
// rides on hue alone (colorblind/grayscale-safe).
const CATEGORY_COLORS: Record<string, string> = {
  security: '#C23237',
  release: '#CFA01F',
  discussion: '#3B9EFF',
  research: '#B658C4',
};
const DEFAULT_CATEGORY_COLOR = '#6B7280';


interface CategoryShape {
  borderRadius: string;
  rotate: boolean;
  donut: boolean;
}

// Distinct silhouettes: circle (discussion), rounded square (release),
// diamond (security — reads as an alert marker), donut (research). All are
// border-radius/rotation based so box-shadow rings follow the shape.
const CATEGORY_SHAPES: Record<string, CategoryShape> = {
  discussion: { borderRadius: '50%', rotate: false, donut: false },
  release: { borderRadius: '22%', rotate: false, donut: false },
  security: { borderRadius: '18%', rotate: true, donut: false },
  research: { borderRadius: '50%', rotate: false, donut: true },
};
const DEFAULT_SHAPE: CategoryShape = CATEGORY_SHAPES.discussion!;

export { CATEGORY_COLORS, CATEGORY_SHAPES };

function getGlowStyle(priority: string | null): string {
  if (priority === 'critical') return '0 0 12px 3px rgba(239, 68, 68, 0.5)';
  if (priority === 'alert') return '0 0 10px 2px rgba(249, 115, 22, 0.4)';
  return 'none';
}

function brighten(hex: string): string {
  const r = Math.min(255, parseInt(hex.slice(1, 3), 16) + 40);
  const g = Math.min(255, parseInt(hex.slice(3, 5), 16) + 40);
  const b = Math.min(255, parseInt(hex.slice(5, 7), 16) + 40);
  return `rgb(${r}, ${g}, ${b})`;
}

const ContentGraphNode = memo(function ContentGraphNode({ id, data, selected }: NodeProps<ContentNode>) {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false);
  const onEnter = useCallback(() => setHovered(true), []);
  const onLeave = useCallback(() => setHovered(false), []);

  const color = CATEGORY_COLORS[data.category] ?? DEFAULT_CATEGORY_COLOR;
  const shape = CATEGORY_SHAPES[data.category] ?? DEFAULT_SHAPE;
  const memberCount = data.member_count ?? 1;
  // Shared with the label-collision resolver so the boxes it places are the
  // boxes that paint (content-graph-label-layout.ts).
  const size = nodeMarkSize(memberCount, data.relevance_score);
  const glow = getGlowStyle(data.signal_priority);
  const extraCount = memberCount - 1;

  // "Touches your stack" is a RING + HALO, never a fill override. The
  // 2026-07-20 white beacon core replaced the category fill, so every stack
  // node rendered as the same plain white circle and lost its category colour
  // — the map's primary channel (audit 2026-10-02). The original objection to
  // a ring (invisible at fit zoom) is answered by sizing: ring and halo widths
  // divide by the live zoom (--graph-zoom, written by ZoomCssVar) so they hold
  // a CONSTANT on-screen size from fit view to close-up. Theme tokens only:
  // the light theme's print-twin gold (--color-accent-gold) engages by itself.
  const isStack = data.affects_you;
  const fill = color;
  const borderColor = brighten(color);
  const stackRing = isStack
    ? `0 0 0 calc(2px / var(--graph-zoom, 1)) var(--color-bg-primary), ` +
      `0 0 0 calc(5px / var(--graph-zoom, 1)) var(--color-accent-gold)`
    : '';
  const stackHalo = isStack
    ? `0 0 calc(20px / var(--graph-zoom, 1)) calc(8px / var(--graph-zoom, 1)) ` +
      `color-mix(in srgb, var(--color-accent-gold) 50%, transparent)`
    : '';
  // Selection ring (detail panel open) sits outside the gold stack ring,
  // separated from it by a bg-primary gap.
  const selectedRing = selected
    ? isStack
      ? `0 0 0 calc(7px / var(--graph-zoom, 1)) var(--color-bg-primary), ` +
        `0 0 0 calc(9px / var(--graph-zoom, 1)) var(--color-text-primary)`
      : `0 0 0 2px var(--color-bg-primary), 0 0 0 4px var(--color-text-primary)`
    : '';
  const boxShadow = [stackRing, selectedRing, stackHalo, glow === 'none' ? '' : glow]
    .filter(Boolean)
    .join(', ') || 'none';
  const shapeTransform = shape.rotate ? ' rotate(45deg)' : '';

  return (
    <div
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      style={{ position: 'relative', width: size, height: size }}
    >
      <Handle
        type="target"
        position={Position.Top}
        style={{ width: 0, height: 0, border: 'none', background: 'transparent' }}
      />

      {data.isNew && (
        <div
          style={{
            position: 'absolute',
            inset: -4,
            borderRadius: shape.borderRadius,
            transform: shape.rotate ? 'rotate(45deg)' : undefined,
            border: `2px solid ${color}`,
            opacity: 0.6,
            animation: 'graph-node-pulse 2s ease-in-out infinite',
          }}
        />
      )}

      {/* The mark carries category (color + silhouette), story mass /
          relevance (size), priority (glow) and stack relevance (gold ring).
          The title lives in the readable label below — text jammed inside a
          28-56px shape was illegible. The label is absolutely positioned so
          the node's measured box stays the mark and edges keep anchoring at
          its center. */}
      <div
        style={{
          width: size,
          height: size,
          borderRadius: shape.borderRadius,
          backgroundColor: fill,
          border: `2px solid ${borderColor}`,
          boxShadow,
          cursor: 'pointer',
          transition: 'transform 150ms ease',
          transform: (hovered ? 'scale(1.15)' : 'scale(1)') + shapeTransform,
        }}
      >
        {shape.donut && (
          <div
            style={{
              position: 'absolute',
              inset: '32%',
              borderRadius: '50%',
              backgroundColor: 'var(--color-bg-primary)',
              pointerEvents: 'none',
            }}
          />
        )}
      </div>

      {extraCount > 0 && (
        <span
          // Painted text the label resolver treats as a hard obstacle
          // (content-graph-label-layout.ts BADGE_*; verified on screen).
          className="cg-node-badge"
          style={{
            position: 'absolute',
            top: -6,
            right: -10,
            padding: '1px 5px',
            borderRadius: 8,
            backgroundColor: 'var(--color-bg-tertiary)',
            border: `1px solid ${brighten(color)}`,
            color: 'var(--color-text-primary)',
            fontSize: 9,
            fontWeight: 600,
            fontFamily: 'JetBrains Mono, monospace',
            lineHeight: 1.4,
            pointerEvents: 'none',
          }}
        >
          {`+${extraCount}`}
        </span>
      )}

      {/* Label size divides by the live zoom (zoomInvariant) so it reads at
          >=11px on screen at fit view; content-graph.css hides non-stack
          labels at far zoom (data-graph-lod) unless hovered — level of
          detail instead of an unreadable smear of 150 titles. Placement
          (below / above / beside the mark) and collision suppression are
          written onto this element by LabelCollisionLayer as
          data-cg-place / data-cg-suppressed, outside React (no re-render
          per zoom step); content-graph.css positions each variant. A
          suppressed label's title stays reachable in the hover tooltip. */}
      <span
        className="cg-node-label"
        data-cg-label-id={`node:${id}`}
        data-stack={isStack ? 'true' : 'false'}
        data-hovered={hovered ? 'true' : 'false'}
        style={{
          width: zoomInvariant(NODE_LABEL_MAX_W),
          color: hovered || isStack ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
          fontSize: zoomInvariant(NODE_LABEL_FONT_PX),
          fontFamily: 'Inter, sans-serif',
          fontWeight: 500,
          lineHeight: NODE_LABEL_LINE,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          pointerEvents: 'none',
          // Halo in the page color keeps labels legible over edge lines.
          textShadow: '0 1px 4px var(--color-bg-primary), 0 0 2px var(--color-bg-primary)',
        }}
      >
        {nodeLabelText(data.title)}
      </span>

      {hovered && (
        <div
          style={{
            position: 'absolute',
            top: size + 6,
            left: '50%',
            // Scale with 1/zoom like the label, so the tooltip is readable at
            // fit view; it opens below the (hidden-while-hovered) label slot.
            transform: 'translateX(-50%) scale(calc(1 / min(var(--graph-zoom, 1), 1)))',
            transformOrigin: 'top center',
            backgroundColor: 'var(--color-bg-tertiary)',
            border: '1px solid var(--color-border)',
            borderRadius: 6,
            padding: '8px 10px',
            zIndex: 50,
            minWidth: 180,
            maxWidth: 280,
            pointerEvents: 'none',
          }}
        >
          <div style={{ color: 'var(--color-text-primary)', fontSize: 12, fontWeight: 600, marginBottom: 4, fontFamily: 'Inter, sans-serif' }}>
            {data.title}
          </div>
          <div style={{ color: 'var(--color-text-secondary)', fontSize: 11, fontFamily: 'Inter, sans-serif' }}>
            {data.source_type}
            {data.signal_type && ` · ${data.signal_type}`}
          </div>
          {data.primary_topic && (
            <div style={{ color: 'var(--color-text-muted)', fontSize: 10, marginTop: 2, fontFamily: 'Inter, sans-serif' }}>
              {data.primary_topic}
            </div>
          )}
          {/* Hover is for scanning; depth lives in the click-through detail
              panel, which lists every member openable. The old per-title dump
              here duplicated the panel and made 25-member advisory storms
              throw a giant hover box over the canvas. */}
          {extraCount > 0 && (
            <div
              style={{
                marginTop: 6,
                paddingTop: 6,
                borderTop: '1px solid var(--color-border)',
                color: 'var(--color-text-secondary)',
                fontSize: 10,
                fontWeight: 600,
                fontFamily: 'Inter, sans-serif',
              }}
            >
              {t('signals.graphStoryMembers', { count: extraCount })}
            </div>
          )}
        </div>
      )}

      <Handle
        type="source"
        position={Position.Bottom}
        style={{ width: 0, height: 0, border: 'none', background: 'transparent' }}
      />
    </div>
  );
});

export default ContentGraphNode;

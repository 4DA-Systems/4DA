// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { Handle, Position, type NodeProps, type Node } from '@xyflow/react';
import { useTranslation } from 'react-i18next';
// Theme overrides for React Flow's chrome (zoom Controls) and the label
// level-of-detail rules. Imported here — the node module is always in the
// graph bundle.
import './content-graph.css';
import type { GraphNode } from '../../types/graph';
import { zoomInvariant } from './graph-zoom';
import { categoryShape, markColor } from './graph-marks';
import {
  NODE_LABEL_FONT_PX,
  NODE_LABEL_LINE,
  NODE_LABEL_MAX_W,
  nodeLabelText,
  nodeMarkSize,
} from './content-graph-label-layout';

export type ContentNodeData = GraphNode & { isNew?: boolean; [key: string]: unknown };
export type ContentNode = Node<ContentNodeData, 'contentNode'>;

function getGlowStyle(priority: string | null): string {
  if (priority === 'critical') return '0 0 12px 3px rgba(239, 68, 68, 0.5)';
  if (priority === 'alert') return '0 0 10px 2px rgba(249, 115, 22, 0.4)';
  return 'none';
}

/**
 * One item. Encoding matches the Themes view (graph-marks.tsx): colour means
 * action — security red, everything else the neutral text tone — and the
 * silhouette carries the category (releases hollow). The old graph painted
 * 87% of nodes one discussion-blue, so colour said nothing (audit
 * 2026-10-06). "Touches your stack" stays a gold ring + halo whose widths
 * divide by the live zoom (--graph-zoom) so it holds a constant on-screen
 * size from fit view to close-up.
 */
const ContentGraphNode = memo(function ContentGraphNode({ id, data, selected }: NodeProps<ContentNode>) {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false);
  const onEnter = useCallback(() => setHovered(true), []);
  const onLeave = useCallback(() => setHovered(false), []);

  const shape = categoryShape(data.category);
  const color = markColor(data.category, false);
  const memberCount = data.member_count ?? 1;
  const size = nodeMarkSize(memberCount, data.relevance_score);
  const glow = getGlowStyle(data.signal_priority);
  const extraCount = memberCount - 1;
  const isStack = data.affects_you;
  const isSecurity = data.category === 'security';

  const stackRing = isStack
    ? `0 0 0 calc(2px / var(--graph-zoom, 1)) var(--color-bg-primary), ` +
      `0 0 0 calc(5px / var(--graph-zoom, 1)) var(--color-accent-gold)`
    : '';
  const stackHalo = isStack
    ? `0 0 calc(20px / var(--graph-zoom, 1)) calc(8px / var(--graph-zoom, 1)) ` +
      `color-mix(in srgb, var(--color-accent-gold) 50%, transparent)`
    : '';
  const selectedRing = selected
    ? isStack
      ? `0 0 0 calc(7px / var(--graph-zoom, 1)) var(--color-bg-primary), ` +
        `0 0 0 calc(9px / var(--graph-zoom, 1)) var(--color-text-primary)`
      : `0 0 0 2px var(--color-bg-primary), 0 0 0 4px var(--color-text-primary)`
    : '';
  const boxShadow = [stackRing, selectedRing, stackHalo, glow === 'none' ? '' : glow].filter(Boolean).join(', ') || 'none';
  const shapeTransform = shape.rotate ? ' rotate(45deg)' : '';

  return (
    <div onMouseEnter={onEnter} onMouseLeave={onLeave} style={{ position: 'relative', width: size, height: size }}>
      <Handle type="target" position={Position.Top} style={{ width: 0, height: 0, border: 'none', background: 'transparent' }} />

      {data.isNew && (
        <div
          style={{
            position: 'absolute',
            inset: -4,
            borderRadius: shape.borderRadius,
            transform: shape.rotate ? 'rotate(45deg)' : undefined,
            border: `2px solid ${isStack ? 'var(--color-accent-gold)' : color}`,
            opacity: 0.6,
            animation: 'graph-node-pulse 2s ease-in-out infinite',
          }}
        />
      )}

      <div
        style={{
          width: size,
          height: size,
          boxSizing: 'border-box',
          borderRadius: shape.borderRadius,
          backgroundColor: shape.hollow ? 'var(--color-bg-primary)' : color,
          border: `${shape.hollow ? Math.max(3, size * 0.14) : 2}px solid ${color}`,
          boxShadow,
          cursor: 'pointer',
          transition: 'transform 150ms ease',
          transform: (hovered ? 'scale(1.15)' : 'scale(1)') + shapeTransform,
          position: 'relative',
        }}
      >
        {shape.donut && (
          <div
            style={{
              position: 'absolute',
              inset: '28%',
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
          // (content-graph-label-layout.ts BADGE_*).
          className="cg-node-badge"
          style={{
            position: 'absolute',
            top: -6,
            right: -10,
            padding: '1px 5px',
            borderRadius: 8,
            backgroundColor: 'var(--color-bg-tertiary)',
            border: '1px solid var(--color-border)',
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

      {/* Zoom-invariant label. At far zoom content-graph.css shows only the
          labels that carry action — stack and security — beside the theme
          headers; LabelCollisionLayer places and suppresses the rest.
          Shrink-to-fit (max-width, not width): the painted box is the text
          box the resolver keeps on-canvas. A fixed 150px box hung up to 75px
          past the text, so an edge label the resolver had flipped fully on
          canvas still measured off it (live 2026-10-07: "uuid v1.27.0" box
          left -14 vs canvas 24 while its text started at 75). */}
      <span
        className="cg-node-label"
        data-cg-label-id={`node:${id}`}
        data-stack={isStack ? 'true' : 'false'}
        data-security={isSecurity ? 'true' : 'false'}
        data-hovered={hovered ? 'true' : 'false'}
        style={{
          maxWidth: zoomInvariant(NODE_LABEL_MAX_W),
          color: hovered || isStack || isSecurity ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
          fontSize: zoomInvariant(NODE_LABEL_FONT_PX),
          fontFamily: 'Inter, sans-serif',
          fontWeight: 500,
          lineHeight: NODE_LABEL_LINE,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          pointerEvents: 'none',
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

      <Handle type="source" position={Position.Bottom} style={{ width: 0, height: 0, border: 'none', background: 'transparent' }} />
    </div>
  );
});

export default ContentGraphNode;

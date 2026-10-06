// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { getBezierPath, BaseEdge, EdgeLabelRenderer, type EdgeProps, type Edge } from '@xyflow/react';

interface ContentEdgeData {
  weight: number;
  label: string | null;
  [key: string]: unknown;
}

type ContentEdge = Edge<ContentEdgeData, 'contentEdge'>;

/** Stroke for "related content" (mutual nearest neighbours — the only edge
 *  the backend emits). Neutral: in this view colour means action (graph-
 *  marks.tsx), and a line between two items is context, not an alert. */
export const EDGE_STROKE = 'var(--color-text-muted)';

const ContentGraphEdge = memo(function ContentGraphEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  style,
}: EdgeProps<ContentEdge>) {
  const [hovered, setHovered] = useState(false);
  const onEnter = useCallback(() => setHovered(true), []);
  const onLeave = useCallback(() => setHovered(false), []);

  const weight = data?.weight ?? 0.5;
  // Hover-focus on a node sets style.opacity = 1 on its edges (the view).
  const restOpacity = Math.min(0.55, 0.15 + weight * 0.4);
  const focusOpacity = (style as { opacity?: number } | undefined)?.opacity;
  const opacity = hovered ? 0.9 : focusOpacity ?? restOpacity;

  const [edgePath, labelX, labelY] = getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition });

  return (
    <>
      {/* Invisible wide path for easier hover targeting */}
      <path d={edgePath} fill="none" stroke="transparent" strokeWidth={20} onMouseEnter={onEnter} onMouseLeave={onLeave} />
      <BaseEdge
        id={id}
        path={edgePath}
        style={{ stroke: EDGE_STROKE, strokeWidth: 1.5, opacity, transition: 'opacity 150ms ease' }}
      />
      {hovered && data?.label && (
        <EdgeLabelRenderer>
          <div
            style={{
              position: 'absolute',
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              backgroundColor: 'var(--color-bg-tertiary)',
              border: '1px solid var(--color-border)',
              borderRadius: 6,
              padding: '4px 8px',
              pointerEvents: 'none',
              zIndex: 50,
              color: 'var(--color-text-secondary)',
              fontSize: 10,
              fontFamily: 'Inter, sans-serif',
            }}
          >
            {data.label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});

export default ContentGraphEdge;

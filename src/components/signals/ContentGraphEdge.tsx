// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { getBezierPath, BaseEdge, EdgeLabelRenderer, type EdgeProps, type Edge } from '@xyflow/react';

interface ContentEdgeData {
  edge_type: string;
  weight: number;
  label: string | null;
  methods: string[];
  [key: string]: unknown;
}

export type ContentEdge = Edge<ContentEdgeData, 'contentEdge'>;

const EDGE_STYLES: Record<string, { color: string; dasharray: string; width: number }> = {
  semantic: { color: '#6366F1', dasharray: 'none', width: 1.5 },
  chain: { color: '#F59E0B', dasharray: '6 3', width: 1.5 },
  concept: { color: '#8B5CF6', dasharray: '2 3', width: 1.5 },
  convergence: { color: '#22C55E', dasharray: 'none', width: 2.5 },
  duplicate: { color: '#EF4444', dasharray: '2 3', width: 1 },
};

const ContentGraphEdge = memo(function ContentGraphEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerEnd,
}: EdgeProps<ContentEdge>) {
  const [hovered, setHovered] = useState(false);
  const onEnter = useCallback(() => setHovered(true), []);
  const onLeave = useCallback(() => setHovered(false), []);

  const edgeType = data?.edge_type ?? 'semantic';
  const weight = data?.weight ?? 0.5;
  const edgeStyle = EDGE_STYLES[edgeType] ?? EDGE_STYLES.semantic!;
  const opacity = Math.min(0.8, 0.3 + weight * 0.5);

  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
  });

  return (
    <>
      {/* Invisible wide path for easier hover targeting */}
      <path
        d={edgePath}
        fill="none"
        stroke="transparent"
        strokeWidth={20}
        onMouseEnter={onEnter}
        onMouseLeave={onLeave}
      />

      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke: edgeStyle.color,
          strokeWidth: edgeStyle.width,
          strokeDasharray: edgeStyle.dasharray,
          opacity: hovered ? Math.min(1, opacity + 0.2) : opacity,
          transition: 'opacity 150ms ease',
        }}
      />

      {hovered && data && (
        <EdgeLabelRenderer>
          <div
            style={{
              position: 'absolute',
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              backgroundColor: 'var(--color-bg-tertiary)',
              border: '1px solid var(--color-border)',
              borderRadius: 6,
              padding: '6px 10px',
              pointerEvents: 'none',
              zIndex: 50,
            }}
          >
            {data.label && (
              <div style={{ color: 'var(--color-text-primary)', fontSize: 11, fontWeight: 600, marginBottom: 2, fontFamily: 'Inter, sans-serif' }}>
                {data.label}
              </div>
            )}
            <div style={{ color: 'var(--color-text-secondary)', fontSize: 10, fontFamily: 'Inter, sans-serif' }}>
              {edgeType} ({(weight * 100).toFixed(0)}%)
            </div>
            {data.methods.length > 0 && (
              <div style={{ color: 'var(--color-text-muted)', fontSize: 9, marginTop: 2, fontFamily: 'JetBrains Mono, monospace' }}>
                {data.methods.join(', ')}
              </div>
            )}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
});

export default ContentGraphEdge;

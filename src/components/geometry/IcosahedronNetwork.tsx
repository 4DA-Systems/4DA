import { useEffect, useCallback, useMemo, useState } from 'react';
import { useGameComponent, type GameElement } from '../../hooks/use-game-component';

interface NetworkNode {
  id: string;
  label: string;
  status?: 'active' | 'syncing' | 'error' | 'offline';
}

interface IcosahedronNetworkProps {
  size?: number;
  nodes?: NetworkNode[];
  highlightIndex?: number;
  fillOpacity?: number;
  rotationSpeed?: number;
  pulse?: number;
}

/**
 * Icosahedron with overlay labels at vertex positions.
 *
 * Computes 2D vertex positions in JS (mirroring the shader math)
 * and positions HTML labels at each projected vertex.
 * Responds to team member data via the nodes prop.
 */
export function IcosahedronNetwork({
  size = 200,
  nodes = [],
  highlightIndex = -1,
  fillOpacity = 0.0,
  rotationSpeed = 0.25,
  pulse = 0.0,
}: IcosahedronNetworkProps) {
  const { containerRef, elementRef } = useGameComponent('game-icosahedron');
  const [projectedPositions, setProjectedPositions] = useState<{ x: number; y: number }[]>([]);

  const setParam = useCallback((name: string, value: number) => {
    (elementRef.current as GameElement)?.setParam?.(name, value);
  }, [elementRef]);

  useEffect(() => {
    setParam('rotation_speed', rotationSpeed);
    setParam('glow_intensity', 1.0);
    setParam('pulse', pulse);
    setParam('fill_opacity', fillOpacity);
    setParam('highlight_vertex', highlightIndex);
    setParam('highlight_color', nodes[highlightIndex]?.status === 'error' ? 1.0 : 0.0);
  }, [rotationSpeed, pulse, fillOpacity, highlightIndex, nodes, setParam]);

  // Compute projected vertex positions in JS (simplified — matches shader at time=0)
  const phi = 1.6180339887;
  const norm = 1.0 / Math.sqrt(1.0 + phi * phi);
  const sc = 0.38;

  // Update projected positions periodically
  useEffect(() => {
    if (nodes.length === 0) return;

    const updatePositions = () => {
      const time = performance.now() / 1000;
      const spd = rotationSpeed;

      // Raw vertices (same as shader)
      const raw: [number, number, number][] = [
        [0, 1, phi], [0, 1, -phi], [0, -1, phi], [0, -1, -phi],
        [1, phi, 0], [1, -phi, 0], [-1, phi, 0], [-1, -phi, 0],
        [phi, 0, 1], [phi, 0, -1], [-phi, 0, 1], [-phi, 0, -1],
      ];

      const verts = raw.map(([x, y, z]) => [x * norm * sc, y * norm * sc, z * norm * sc]);

      // Simplified rotation (Y-axis only for label tracking)
      const angle = time * spd;
      const ca = Math.cos(angle);
      const sa = Math.sin(angle);

      const projected = verts.map(([x, y, z]) => {
        const rx = ca * x + sa * z;
        const rz = -sa * x + ca * z;
        const d = 3.5;
        const s = d / (d - rz);
        return { x: rx * s, y: y * s };
      });

      // Map from [-0.5, 0.5] normalized coords to pixel coords
      const half = size / 2;
      setProjectedPositions(projected.map(p => ({
        x: half + p.x * half * 1.8,
        y: half - p.y * half * 1.8,
      })));
    };

    const interval = setInterval(updatePositions, 100); // 10fps for labels
    updatePositions();
    return () => clearInterval(interval);
  }, [nodes.length, rotationSpeed, size, norm, sc]);

  const visibleNodes = useMemo(() =>
    nodes.slice(0, 12), // max 12 nodes (icosahedron vertices)
  [nodes]);

  return (
    <div style={{ width: size, height: size, position: 'relative' }}>
      <div ref={containerRef} style={{ width: '100%', height: '100%' }} />

      {/* Vertex labels overlay */}
      {visibleNodes.map((node, i) => {
        const pos = projectedPositions[i];
        if (!pos) return null;
        const statusColor = node.status === 'error' ? '#EF4444'
          : node.status === 'syncing' ? '#D4AF37'
          : node.status === 'active' ? '#22C55E'
          : '#8A8A8A';

        return (
          <div
            key={node.id}
            style={{
              position: 'absolute',
              left: pos.x - 8,
              top: pos.y - 8,
              width: 16,
              height: 16,
              borderRadius: '50%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              pointerEvents: 'none',
              transition: 'left 0.1s, top 0.1s',
            }}
            title={`${node.label} (${node.status || 'unknown'})`}
          >
            <span
              style={{
                fontSize: 7,
                fontFamily: 'JetBrains Mono, monospace',
                color: statusColor,
                fontWeight: 600,
                textShadow: '0 0 4px rgba(0,0,0,0.8)',
                letterSpacing: '0.05em',
              }}
            >
              {node.label.slice(0, 2).toUpperCase()}
            </span>
          </div>
        );
      })}
    </div>
  );
}

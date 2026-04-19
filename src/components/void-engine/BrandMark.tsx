// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, useId, useEffect, useRef, useState, useCallback } from 'react';
import type { VoidSignal } from '../../types';

// ---------------------------------------------------------------------------
// 3D tetrahedron geometry
// ---------------------------------------------------------------------------

// Regular tetrahedron inscribed in the unit sphere (vertices on r=1)
const TETRA_VERTS: [number, number, number][] = [
  [0, 1, 0],                       // apex
  [0.9428, -0.3333, 0],            // front-right
  [-0.4714, -0.3333, 0.8165],      // back-left
  [-0.4714, -0.3333, -0.8165],     // back-right
];

// All 6 edges (complete graph K4)
const TETRA_EDGES: [number, number][] = [
  [0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3],
];

// 4 triangular faces (vertex index triples, wound consistently)
const TETRA_FACES: [number, number, number][] = [
  [0, 1, 2], // apex-front-left
  [0, 2, 3], // apex-left-back
  [0, 3, 1], // apex-back-front
  [1, 3, 2], // base
];

// ---------------------------------------------------------------------------
// 3D math
// ---------------------------------------------------------------------------

function rotY(x: number, y: number, z: number, a: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  return [c * x + s * z, y, -s * x + c * z];
}

function rotX(x: number, y: number, z: number, a: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  return [x, c * y - s * z, s * y + c * z];
}

function project(
  x: number, y: number, z: number,
  camDist: number, scale: number, cx: number, cy: number,
): [number, number, number] {
  const w = camDist / (camDist - z);
  return [cx + x * scale * w, cy - y * scale * w, z];
}

/** Face normal Z-component (positive = facing camera). Used for backface test. */
function faceNormalZ(
  ax: number, ay: number, bx: number, by: number, cx: number, cy: number,
): number {
  return (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface BrandMarkProps {
  signal?: VoidSignal;
  size?: number;
}

type ProjVert = { x: number; y: number; z: number };
type ProjEdge = { x1: number; y1: number; x2: number; y2: number; depth: number };
type ProjFace = { points: string; depth: number; facing: number };

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * 4DA brand mark — 3D rotating tetrahedron.
 *
 * 4 vertices, 6 edges, 4 faces. Real 3D geometry with compound rotation,
 * perspective projection, depth-sorted face fills, and depth-scaled edges.
 * Signal-responsive: color, glow, and rotation speed.
 */
export function BrandMark({ signal, size = 36 }: BrandMarkProps) {
  const filterId = useId().replace(/:/g, '');

  // Derive visual state from signal
  const { glowOpacity, edgeColor, vertexColor, faceColor, stateLabel, rotSpeed } = useMemo(() => {
    if (!signal) {
      return {
        glowOpacity: 0.15,
        edgeColor: '#C8B560',
        vertexColor: '#D4AF37',
        faceColor: '#D4AF37',
        stateLabel: 'Idle',
        rotSpeed: 0.014,
      };
    }

    const glow = signal.error > 0.5
      ? 0.05
      : 0.12 + signal.heat * 0.15 + signal.pulse * 0.1 + signal.burst * 0.2;

    let edge = '#C8B560';
    let vertex = '#D4AF37';
    let face = '#D4AF37';
    if (signal.error > 0.5 || signal.critical_count > 0) {
      edge = '#EF4444'; vertex = '#F87171'; face = '#EF4444';
    } else if (signal.signal_color_shift > 0.5) {
      edge = '#F59E0B'; vertex = '#FBBF24'; face = '#F59E0B';
    } else if (signal.signal_color_shift < -0.3) {
      edge = '#6B93C0'; vertex = '#7BA7D4'; face = '#6B93C0';
    }

    let label = 'Idle';
    if (signal.critical_count > 0 && signal.signal_intensity > 0.75) {
      label = signal.critical_count > 1 ? `${signal.critical_count} Alerts` : 'Alert';
    } else if (signal.signal_color_shift > 0.5) {
      label = 'Breaking';
    } else if (signal.signal_color_shift > 0.2) {
      label = 'Discovery';
    } else if (signal.signal_color_shift < -0.3) {
      label = 'Learning';
    } else if (signal.morph > 0.3) {
      label = 'Context';
    } else if (signal.signal_urgency > 0.6) {
      label = 'Urgent';
    } else if (signal.item_count === 0 && signal.heat === 0) {
      label = signal.staleness > 0.9 ? 'Dormant' : 'Awakening';
    } else if (signal.error > 0.5) {
      label = 'Error';
    } else if (signal.staleness > 0.8) {
      label = 'Stale';
    } else if (signal.pulse > 0.5) {
      label = 'Scanning';
    } else if (signal.heat > 0.5) {
      label = 'Discoveries';
    } else if (signal.item_count > 0) {
      label = 'Active';
    }

    let speed = 2 * Math.PI / (60 * 30);
    if (signal.error > 0.5) {
      speed = 2 * Math.PI / (60 * 30);
    } else if (signal.pulse > 0.5) {
      speed = 2 * Math.PI / (18 * 30);
    } else if (signal.heat > 0.3 || signal.signal_intensity > 0.4) {
      speed = 2 * Math.PI / (24 * 30);
    } else if (signal.item_count > 0) {
      speed = 2 * Math.PI / (36 * 30);
    } else if (signal.staleness > 0.9) {
      speed = 2 * Math.PI / (90 * 30);
    }

    return {
      glowOpacity: glow, edgeColor: edge, vertexColor: vertex,
      faceColor: face, stateLabel: label, rotSpeed: speed,
    };
  }, [signal]);

  // ---------------------------------------------------------------------------
  // 3D animation loop
  // ---------------------------------------------------------------------------
  const angleYRef = useRef(0);
  const frameRef = useRef(0); // monotonic frame counter for secondary motion
  const speedRef = useRef(rotSpeed);
  speedRef.current = rotSpeed;
  const rafRef = useRef(0);

  const [verts, setVerts] = useState<ProjVert[]>([]);
  const [edges, setEdges] = useState<ProjEdge[]>([]);
  const [faces, setFaces] = useState<ProjFace[]>([]);

  const computeFrame = useCallback(() => {
    const angleY = angleYRef.current;
    const frame = frameRef.current;
    const scale = 37;
    const camDist = 4.8;
    const cx = 50;
    const cy = 50;

    // Compound rotation: primary Y + slow drifting X tilt (organic, not mechanical)
    // X oscillates between ~15° and ~25° over ~40 seconds at 30fps
    const tiltX = 0.35 + Math.sin(frame * 0.0026) * 0.09;

    // Project all vertices
    const projected: ProjVert[] = TETRA_VERTS.map(([vx, vy, vz]) => {
      const [rx, ry, rz] = rotY(vx, vy, vz, angleY);
      const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
      const [px, py, pz] = project(tx, ty, tz, camDist, scale, cx, cy);
      return { x: px, y: py, z: pz };
    });

    // Faces — sorted back-to-front by centroid depth
    const projFaces: ProjFace[] = TETRA_FACES.map(([a, b, c]) => {
      // Indices are constants 0-3, always in range — assert for TS
      const va = projected[a]!;
      const vb = projected[b]!;
      const vc = projected[c]!;
      const points = `${va.x},${va.y} ${vb.x},${vb.y} ${vc.x},${vc.y}`;
      const depth = (va.z + vb.z + vc.z) / 3;
      const facing = faceNormalZ(va.x, va.y, vb.x, vb.y, vc.x, vc.y);
      return { points, depth, facing };
    });
    projFaces.sort((a, b) => a.depth - b.depth);

    // Edges — sorted back-to-front
    const projEdges: ProjEdge[] = TETRA_EDGES.map(([a, b]) => {
      const pa = projected[a]!;
      const pb = projected[b]!;
      return {
        x1: pa.x, y1: pa.y,
        x2: pb.x, y2: pb.y,
        depth: (pa.z + pb.z) / 2,
      };
    });
    projEdges.sort((a, b) => a.depth - b.depth);

    setVerts(projected);
    setEdges(projEdges);
    setFaces(projFaces);
  }, []);

  useEffect(() => {
    computeFrame();

    let lastTime = 0;
    const FRAME_MS = 1000 / 30;

    const loop = (time: number) => {
      rafRef.current = requestAnimationFrame(loop);
      if (time - lastTime < FRAME_MS) return;
      lastTime = time;

      angleYRef.current += speedRef.current;
      frameRef.current += 1;
      computeFrame();
    };

    rafRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(rafRef.current);
  }, [computeFrame]);

  // ---------------------------------------------------------------------------
  // Render helpers
  // ---------------------------------------------------------------------------

  // Depth brightness: 0.25 (far) to 1.0 (near)
  const depthBright = (z: number) => 0.25 + 0.75 * ((z + 1.2) / 2.4);

  // Edge stroke width scales with depth — near edges thicker
  const baseStroke = size <= 24 ? 3.5 : size <= 48 ? 2.8 : 2;
  const edgeStroke = (z: number) => baseStroke * (0.6 + 0.4 * depthBright(z));

  // Vertex radius scales with depth
  const baseVtx = size <= 24 ? 5.5 : size <= 48 ? 4.5 : 3.5;

  // Face fill opacity: front-facing = brighter, back-facing = dimmer
  const faceOpacity = (depth: number, facing: number) => {
    const base = 0.03 + 0.05 * depthBright(depth);
    return facing > 0 ? base * 1.5 : base * 0.5;
  };

  const showLabel = size >= 100;

  const itemCount = signal?.item_count ?? 0;
  const openWindows = signal?.open_windows ?? 0;

  const titleParts = [`4DA: ${stateLabel}`];
  if (itemCount > 0) titleParts.push(`${itemCount} items`);
  if (openWindows > 0) titleParts.push(`${openWindows} decision window${openWindows > 1 ? 's' : ''}`);

  const ariaLabel = `4DA status: ${stateLabel}${itemCount > 0 ? `, ${itemCount} items found` : ''}`;

  return (
    <div
      className="brand-mark-container"
      role="status"
      aria-live="polite"
      title={titleParts.join(' \u00b7 ')}
      aria-label={ariaLabel}
      style={{
        width: size,
        height: size,
        position: 'relative',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style={{ display: 'block' }}
      >
        <defs>
          <filter id={`glow-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Face fills — semi-transparent, sorted back-to-front. Gives mass. */}
        <g>
          {faces.map((f, i) => (
            <polygon
              key={`f${i}`}
              points={f.points}
              fill={faceColor}
              opacity={faceOpacity(f.depth, f.facing)}
            />
          ))}
        </g>

        {/* Edge glow layer */}
        <g opacity={glowOpacity} filter={`url(#glow-${filterId})`}>
          {edges.map((e, i) => (
            <line
              key={`g${i}`}
              x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
              stroke={vertexColor}
              strokeWidth={edgeStroke(e.depth) + 1.5}
              strokeLinecap="round"
              opacity={depthBright(e.depth)}
            />
          ))}
        </g>

        {/* Sharp edge layer — depth-sorted, width + brightness by depth */}
        <g>
          {edges.map((e, i) => (
            <line
              key={`e${i}`}
              x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
              stroke={edgeColor}
              strokeWidth={edgeStroke(e.depth)}
              strokeLinecap="round"
              opacity={depthBright(e.depth)}
            />
          ))}
        </g>

        {/* Vertex dots — near vertices draw on top, sized by depth */}
        <g>
          {[...verts]
            .map((v, i) => ({ ...v, i }))
            .sort((a, b) => a.z - b.z)
            .map((v) => {
              const b = depthBright(v.z);
              return (
                <circle
                  key={`v${v.i}`}
                  cx={v.x}
                  cy={v.y}
                  r={baseVtx * (0.6 + 0.4 * b)}
                  fill={vertexColor}
                  opacity={b}
                />
              );
            })}
        </g>
      </svg>

      {showLabel && (
        <span
          className="brand-mark-label"
          style={{
            position: 'absolute',
            bottom: 8,
            fontSize: 10,
            color: (signal?.error ?? 0) > 0.5 || (signal?.critical_count ?? 0) > 0
              ? 'var(--color-error)'
              : (signal?.signal_color_shift ?? 0) > 0.5
                ? 'var(--color-accent-gold)'
                : (signal?.signal_color_shift ?? 0) < -0.3
                  ? '#4A90D9'
                  : 'var(--color-text-muted)',
            letterSpacing: '0.1em',
            textTransform: 'uppercase',
            fontFamily: 'JetBrains Mono, monospace',
            opacity: 0.6,
            transition: 'color 0.3s ease',
          }}
        >
          {stateLabel}
        </span>
      )}
    </div>
  );
}

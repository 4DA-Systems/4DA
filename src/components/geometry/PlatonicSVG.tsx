// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useId, useRef, useState } from 'react';

interface PlatonicSVGProps {
  vertices: [number, number, number][];
  edges: [number, number][];
  faces: number[][];
  size?: number;
  color?: string;
  glowColor?: string;
  rotationSpeed?: number;
  secondaryTilt?: boolean;
  className?: string;
}

type PV = { x: number; y: number; z: number };
type PE = { x1: number; y1: number; x2: number; y2: number; depth: number };
type PF = { points: string; depth: number; facing: number };

function rotY(x: number, y: number, z: number, a: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  return [c * x + s * z, y, -s * x + c * z];
}

function rotX(x: number, y: number, z: number, a: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  return [x, c * y - s * z, s * y + c * z];
}

function proj(
  x: number, y: number, z: number,
  cam: number, sc: number, cx: number, cy: number,
): [number, number, number] {
  const w = cam / (cam - z);
  return [cx + x * sc * w, cy - y * sc * w, z];
}

function faceNZ(ax: number, ay: number, bx: number, by: number, cx: number, cy: number): number {
  return (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
}

/**
 * Generic platonic solid renderer — BrandMark quality for any polyhedron.
 *
 * 4-layer rendering pipeline:
 *   1. Face fills — back-to-front, semi-transparent, front-facing brighter
 *   2. Edge glow — gaussian blur, depth-weighted
 *   3. Sharp edges — crisp strokes, depth-scaled width + opacity
 *   4. Vertex dots — depth-sorted, radius + opacity scale with z
 *
 * Organic compound rotation: primary Y + slow oscillating X tilt.
 */
export function PlatonicSVG({
  vertices,
  edges,
  faces,
  size = 180,
  color = '#C8B560',
  glowColor = '#D4AF37',
  rotationSpeed = 0.012,
  secondaryTilt = true,
  className,
}: PlatonicSVGProps) {
  const filterId = useId().replace(/:/g, '');
  const angleRef = useRef(0);
  const frameRef = useRef(0);
  const rafRef = useRef(0);

  const [pVerts, setPVerts] = useState<PV[]>([]);
  const [pEdges, setPEdges] = useState<PE[]>([]);
  const [pFaces, setPFaces] = useState<PF[]>([]);

  const computeFrame = useCallback(() => {
    const ay = angleRef.current;
    const f = frameRef.current;
    const scale = 37;
    const cam = 5.2;
    const cx = 50, cy = 50;

    const tiltX = secondaryTilt ? 0.35 + Math.sin(f * 0.002) * 0.12 : 0.35;

    const projected: PV[] = vertices.map(([vx, vy, vz]) => {
      const [rx, ry, rz] = rotY(vx, vy, vz, ay);
      const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
      const [px, py, pz] = proj(tx, ty, tz, cam, scale, cx, cy);
      return { x: px, y: py, z: pz };
    });

    const projFaces: PF[] = faces.map(faceIndices => {
      const fvs = faceIndices.map(i => projected[i]!);
      const pts = fvs.map(v => `${v.x},${v.y}`).join(' ');
      const depth = fvs.reduce((s, v) => s + v.z, 0) / fvs.length;
      const facing = fvs.length >= 3
        ? faceNZ(fvs[0]!.x, fvs[0]!.y, fvs[1]!.x, fvs[1]!.y, fvs[2]!.x, fvs[2]!.y)
        : 1;
      return { points: pts, depth, facing };
    });
    projFaces.sort((a, b) => a.depth - b.depth);

    const projEdges: PE[] = edges.map(([a, b]) => {
      const pa = projected[a]!, pb = projected[b]!;
      return { x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y, depth: (pa.z + pb.z) / 2 };
    });
    projEdges.sort((a, b) => a.depth - b.depth);

    setPVerts(projected);
    setPEdges(projEdges);
    setPFaces(projFaces);
  }, [vertices, edges, faces, secondaryTilt]);

  useEffect(() => {
    computeFrame();
    let lastTime = 0;
    const FRAME_MS = 1000 / 30;

    const loop = (time: number) => {
      rafRef.current = requestAnimationFrame(loop);
      if (time - lastTime < FRAME_MS) return;
      lastTime = time;
      angleRef.current += rotationSpeed;
      frameRef.current += 1;
      computeFrame();
    };

    rafRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(rafRef.current);
  }, [computeFrame, rotationSpeed]);

  const depthBright = (z: number) => 0.25 + 0.75 * ((z + 1.2) / 2.4);
  const baseStroke = 2.2;
  const edgeStroke = (z: number) => baseStroke * (0.6 + 0.4 * depthBright(z));
  const baseVtx = 3.5;

  const faceOpacity = (depth: number, facing: number) => {
    const base = 0.03 + 0.04 * depthBright(depth);
    return facing > 0 ? base * 1.5 : base * 0.5;
  };

  return (
    <div className={className} style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <filter id={`pg-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2.5" result="blur" />
            <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
          </filter>
        </defs>

        {/* Layer 1: Face fills */}
        <g>
          {pFaces.map((f, i) => (
            <polygon key={`f${i}`} points={f.points} fill={glowColor} opacity={faceOpacity(f.depth, f.facing)} />
          ))}
        </g>

        {/* Layer 2: Edge glow */}
        <g opacity={0.15} filter={`url(#pg-${filterId})`}>
          {pEdges.map((e, i) => (
            <line key={`g${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
              stroke={glowColor} strokeWidth={edgeStroke(e.depth) + 1.5}
              strokeLinecap="round" opacity={depthBright(e.depth)} />
          ))}
        </g>

        {/* Layer 3: Sharp edges */}
        <g>
          {pEdges.map((e, i) => (
            <line key={`e${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
              stroke={color} strokeWidth={edgeStroke(e.depth)}
              strokeLinecap="round" opacity={depthBright(e.depth)} />
          ))}
        </g>

        {/* Layer 4: Vertex dots */}
        <g>
          {[...pVerts].map((v, i) => ({ ...v, i })).sort((a, b) => a.z - b.z).map(v => {
            const b = depthBright(v.z);
            return <circle key={`v${v.i}`} cx={v.x} cy={v.y} r={baseVtx * (0.6 + 0.4 * b)} fill={glowColor} opacity={b} />;
          })}
        </g>
      </svg>
    </div>
  );
}

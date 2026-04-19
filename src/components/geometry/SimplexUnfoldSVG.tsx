// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useId, useRef, useState } from 'react';

type PV = { x: number; y: number; z: number };
type PE = { x1: number; y1: number; x2: number; y2: number; depth: number };

function rotY(x: number, y: number, z: number, a: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  return [c * x + s * z, y, -s * x + c * z];
}
function rotX(x: number, y: number, z: number, a: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  return [x, c * y - s * z, s * y + c * z];
}
function proj(x: number, y: number, z: number, cam: number, sc: number, cx: number, cy: number): [number, number, number] {
  const w = cam / (cam - z);
  return [cx + x * sc * w, cy - y * sc * w, z];
}

// Simplex vertices by dimension (each adds one vertex connected to all prior)
const PHASE_VERTS: [number, number, number][][] = [
  // 0D: single point at origin
  [[0, 0, 0]],
  // 1D: line segment
  [[0, 0.6, 0], [0, -0.6, 0]],
  // 2D: equilateral triangle
  [[0, 0.7, 0], [0.606, -0.35, 0], [-0.606, -0.35, 0]],
  // 3D: tetrahedron
  [[0, 1, 0], [0.943, -0.333, 0], [-0.471, -0.333, 0.817], [-0.471, -0.333, -0.817]],
  // 4D: pentachoron (projected — 5th vertex pulsing in/out along z)
  [[0, 0.85, 0], [0.8, -0.28, 0], [-0.4, -0.28, 0.69], [-0.4, -0.28, -0.69], [0, 0.1, 0.5]],
];

function lerp3(a: [number, number, number], b: [number, number, number], t: number): [number, number, number] {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

function ease(t: number) {
  return t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t;
}

interface SimplexUnfoldSVGProps {
  size?: number;
  color?: string;
  glowColor?: string;
  className?: string;
}

/**
 * Simplex unfold — dimensional emergence animation.
 * Point → Line → Triangle → Tetrahedron → Pentachoron.
 * Each phase morphs smoothly via eased vertex interpolation.
 * BrandMark 4-layer rendering at each phase.
 */
export function SimplexUnfoldSVG({
  size = 180,
  color = '#C8B560',
  glowColor = '#D4AF37',
  className,
}: SimplexUnfoldSVGProps) {
  const filterId = useId().replace(/:/g, '');
  const frameRef = useRef(0);
  const rafRef = useRef(0);

  const [pVerts, setPVerts] = useState<PV[]>([]);
  const [pEdges, setPEdges] = useState<PE[]>([]);

  const PHASE_DURATION = 120; // frames per phase
  const MORPH_DURATION = 40; // frames for morph transition
  const TOTAL_CYCLE = PHASE_DURATION * 5;

  const computeFrame = useCallback(() => {
    const f = frameRef.current;
    const scale = 32;
    const cam = 5;
    const cx = 50, cy = 50;

    const cycleFrame = f % TOTAL_CYCLE;
    const phaseIdx = Math.floor(cycleFrame / PHASE_DURATION);
    const phaseProgress = (cycleFrame % PHASE_DURATION);

    // Current vertices: morph from prev phase to current
    const fromPhase = phaseIdx === 0 ? 4 : phaseIdx - 1;
    const toPhase = phaseIdx;
    const fromVerts = PHASE_VERTS[fromPhase]!;
    const toVerts = PHASE_VERTS[toPhase]!;

    let morphT: number;
    if (phaseProgress < MORPH_DURATION) {
      morphT = ease(phaseProgress / MORPH_DURATION);
    } else {
      morphT = 1;
    }

    // Interpolate vertices — handle different counts by spawning from center
    const nVerts = Math.max(fromVerts.length, toVerts.length);
    const currentVerts: [number, number, number][] = [];
    for (let i = 0; i < nVerts; i++) {
      const from = fromVerts[Math.min(i, fromVerts.length - 1)]!;
      const to = toVerts[Math.min(i, toVerts.length - 1)]!;
      currentVerts.push(lerp3(from, to, morphT));
    }

    // Edges: complete graph on current vertex count (simplex property)
    const currentEdges: [number, number][] = [];
    for (let i = 0; i < toVerts.length; i++) {
      for (let j = i + 1; j < toVerts.length; j++) {
        currentEdges.push([i, j]);
      }
    }

    // Animate rotation
    const ay = f * 0.008;
    const tiltX = 0.3 + Math.sin(f * 0.002) * 0.1;

    const projected: PV[] = currentVerts.map(([vx, vy, vz]) => {
      const [rx, ry, rz] = rotY(vx, vy, vz, ay);
      const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
      const [px, py, pz] = proj(tx, ty, tz, cam, scale, cx, cy);
      return { x: px, y: py, z: pz };
    });

    const projEdges: PE[] = currentEdges.map(([a, b]) => {
      const pa = projected[a]!, pb = projected[b]!;
      return { x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y, depth: (pa.z + pb.z) / 2 };
    });
    projEdges.sort((a, b) => a.depth - b.depth);

    setPVerts(projected);
    setPEdges(projEdges);
  }, [TOTAL_CYCLE]);

  useEffect(() => {
    computeFrame();
    let lastTime = 0;
    const FRAME_MS = 1000 / 30;
    const loop = (time: number) => {
      rafRef.current = requestAnimationFrame(loop);
      if (time - lastTime < FRAME_MS) return;
      lastTime = time;
      frameRef.current += 1;
      computeFrame();
    };
    rafRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(rafRef.current);
  }, [computeFrame]);

  const depthBright = (z: number) => 0.25 + 0.75 * ((z + 1.2) / 2.4);
  const edgeStroke = (z: number) => 2.2 * (0.6 + 0.4 * depthBright(z));

  return (
    <div className={className} style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <filter id={`su-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2.5" result="blur" />
            <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
          </filter>
        </defs>
        <g opacity={0.15} filter={`url(#su-${filterId})`}>
          {pEdges.map((e, i) => <line key={`g${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2} stroke={glowColor} strokeWidth={edgeStroke(e.depth) + 1.5} strokeLinecap="round" opacity={depthBright(e.depth)} />)}
        </g>
        <g>{pEdges.map((e, i) => <line key={`e${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2} stroke={color} strokeWidth={edgeStroke(e.depth)} strokeLinecap="round" opacity={depthBright(e.depth)} />)}</g>
        <g>{[...pVerts].map((v, i) => ({ ...v, i })).sort((a, b) => a.z - b.z).map(v => {
          const b = depthBright(v.z);
          return <circle key={`v${v.i}`} cx={v.x} cy={v.y} r={4 * (0.5 + 0.5 * b)} fill={glowColor} opacity={b} />;
        })}</g>
      </svg>
    </div>
  );
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useId, useRef, useState } from 'react';

interface LogoMarkSVGProps {
  size?: number;
  className?: string;
}

// "4" numeral vertices in normalized 3D coordinates
// Clean geometric "4": diagonal + crossbar + vertical
//     A
//    /|
//   / |
//  C--E--D
//     |
//     B
const VERTICES: [number, number, number][] = [
  [0.07, 0.39, 0.0],   // A: top of vertical
  [0.07, -0.39, 0.0],  // B: bottom of vertical
  [-0.29, -0.03, 0.0], // C: left of crossbar
  [0.29, -0.03, 0.0],  // D: right of crossbar
  [0.07, -0.03, 0.0],  // E: junction
];

const EDGES: [number, number][] = [
  [2, 0], // C→A diagonal
  [2, 4], // C→E crossbar left
  [4, 3], // E→D crossbar right
  [0, 1], // A→B vertical
];

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

function proj(
  x: number, y: number, z: number,
  cam: number, sc: number, cx: number, cy: number,
): [number, number, number] {
  const w = cam / (cam - z);
  return [cx + x * sc * w, cy - y * sc * w, z];
}

export function LogoMarkSVG({ size = 112, className }: LogoMarkSVGProps) {
  const filterId = useId().replace(/:/g, '');
  const angleRef = useRef(0);
  const angularVelRef = useRef(0.04);
  const prevMouseXRef = useRef<number | null>(null);
  const frameRef = useRef(0);
  const rafRef = useRef(0);
  const containerRef = useRef<HTMLDivElement>(null);

  const [pVerts, setPVerts] = useState<PV[]>([]);
  const [pEdges, setPEdges] = useState<PE[]>([]);

  const color = '#C8B560';
  const glowColor = '#D4AF37';
  const FRICTION = 0.975;
  const SPRING_K = 0.03;

  const computeFrame = useCallback(() => {
    const ay = angleRef.current;
    const f = frameRef.current;
    const scale = 80;
    const cam = 5.2;
    const cx = 50, cy = 50;
    const tiltX = Math.sin(f * 0.003) * 0.10;

    const projected: PV[] = VERTICES.map(([vx, vy, vz]) => {
      const [rx, ry, rz] = rotY(vx, vy, vz, ay);
      const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
      const [px, py, pz] = proj(tx, ty, tz, cam, scale, cx, cy);
      return { x: px, y: py, z: pz };
    });

    const projEdges: PE[] = EDGES.map(([a, b]) => {
      const pa = projected[a]!, pb = projected[b]!;
      return { x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y, depth: (pa.z + pb.z) / 2 };
    });
    projEdges.sort((a, b) => a.depth - b.depth);

    setPVerts(projected);
    setPEdges(projEdges);
  }, []);

  useEffect(() => {
    computeFrame();
    let lastTime = 0;
    const FRAME_MS = 1000 / 30;

    const loop = (time: number) => {
      rafRef.current = requestAnimationFrame(loop);
      if (time - lastTime < FRAME_MS) return;
      lastTime = time;

      angularVelRef.current *= FRICTION;

      if (Math.abs(angularVelRef.current) < 0.01) {
        const nearest = Math.round(angleRef.current / (Math.PI * 2)) * Math.PI * 2;
        angularVelRef.current += (nearest - angleRef.current) * SPRING_K;
      }

      angleRef.current += angularVelRef.current;
      frameRef.current += 1;
      computeFrame();
    };

    rafRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(rafRef.current);
  }, [computeFrame]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const onMove = (e: MouseEvent) => {
      const rect = el.getBoundingClientRect();
      const mx = (e.clientX - rect.left) / rect.width;
      if (prevMouseXRef.current !== null) {
        const dx = mx - prevMouseXRef.current;
        if (Math.abs(dx) > 0.001) {
          angularVelRef.current += dx * 2.0;
        }
      }
      prevMouseXRef.current = mx;
    };

    const onLeave = () => { prevMouseXRef.current = null; };

    el.addEventListener('mousemove', onMove);
    el.addEventListener('mouseleave', onLeave);
    return () => {
      el.removeEventListener('mousemove', onMove);
      el.removeEventListener('mouseleave', onLeave);
    };
  }, []);

  const depthBright = (z: number) => 0.3 + 0.7 * Math.min(1, Math.max(0, (z + 0.35) / 0.7));
  const baseStroke = 2.8;
  const edgeStroke = (z: number) => baseStroke * (0.6 + 0.4 * depthBright(z));
  const baseVtx = 4.2;

  return (
    <div
      ref={containerRef}
      className={className}
      style={{ width: size, height: size, cursor: 'grab' }}
      role="img"
      aria-label="4DA logo"
    >
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <filter id={`lm-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2.5" result="blur" />
            <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
          </filter>
        </defs>

        {/* Edge glow */}
        <g opacity={0.2} filter={`url(#lm-${filterId})`}>
          {pEdges.map((e, i) => (
            <line key={`g${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
              stroke={glowColor} strokeWidth={edgeStroke(e.depth) + 2}
              strokeLinecap="round" opacity={depthBright(e.depth)} />
          ))}
        </g>

        {/* Sharp edges */}
        <g>
          {pEdges.map((e, i) => (
            <line key={`e${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2}
              stroke={color} strokeWidth={edgeStroke(e.depth)}
              strokeLinecap="round" opacity={depthBright(e.depth)} />
          ))}
        </g>

        {/* Vertex dots */}
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

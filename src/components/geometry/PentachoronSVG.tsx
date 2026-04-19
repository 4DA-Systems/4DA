// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { PENTACHORON } from './geometries';

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

function proj(x: number, y: number, z: number, cam: number, sc: number, cx: number, cy: number): [number, number, number] {
  const w = cam / (cam - z);
  return [cx + x * sc * w, cy - y * sc * w, z];
}

function faceNZ(ax: number, ay: number, bx: number, by: number, cx: number, cy: number) {
  return (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
}

// 4D rotation in XW and YZ planes — two golden-ratio-related speeds
function rot4D(v: [number, number, number, number], a1: number, a2: number): [number, number, number, number] {
  const [x, y, z, w] = v;
  const c1 = Math.cos(a1), s1 = Math.sin(a1);
  const c2 = Math.cos(a2), s2 = Math.sin(a2);
  const x2 = c1 * x + s1 * w;
  const w2 = -s1 * x + c1 * w;
  const y2 = c2 * y + s2 * z;
  const z2 = -s2 * y + c2 * z;
  return [x2, y2, z2, w2];
}

// 4D→3D stereographic-style perspective projection
function proj4Dto3D(v: [number, number, number, number], cam4D: number): [number, number, number] {
  const w = cam4D / (cam4D - v[3]);
  return [v[0] * w, v[1] * w, v[2] * w];
}

interface PentachoronSVGProps {
  size?: number;
  color?: string;
  glowColor?: string;
  className?: string;
}

/**
 * 4D pentachoron (5-cell) renderer — BrandMark quality with 4D rotation.
 *
 * Two 4D rotation planes spin at golden-ratio-related speeds (0.618 / 0.382)
 * before 4D→3D stereographic projection. Then the standard BrandMark
 * 4-layer rendering pipeline runs on the 3D result.
 */
export function PentachoronSVG({
  size = 180,
  color = '#B8A860',
  glowColor = '#C7B86E',
  className,
}: PentachoronSVGProps) {
  const filterId = useId().replace(/:/g, '');
  const frameRef = useRef(0);
  const rafRef = useRef(0);

  const [pVerts, setPVerts] = useState<PV[]>([]);
  const [pEdges, setPEdges] = useState<PE[]>([]);
  const [pFaces, setPFaces] = useState<PF[]>([]);

  const PHI = (1 + Math.sqrt(5)) / 2;
  const speed1 = 0.008 / PHI;
  const speed2 = 0.008 * (2 - PHI);

  const computeFrame = useCallback(() => {
    const f = frameRef.current;
    const scale = 30;
    const cam = 5.5;
    const cam4D = 3.5;
    const cx = 50, cy = 50;

    const a4d1 = f * speed1;
    const a4d2 = f * speed2;
    const tiltX = 0.3 + Math.sin(f * 0.0018) * 0.1;
    const rotYAngle = f * 0.008;

    // 4D rotate → 4D→3D project → 3D rotate → 2D project
    const projected: PV[] = PENTACHORON.vertices.map(v4 => {
      const r4 = rot4D(v4, a4d1, a4d2);
      const [vx, vy, vz] = proj4Dto3D(r4, cam4D);
      const [rx, ry, rz] = rotY(vx, vy, vz, rotYAngle);
      const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
      const [px, py, pz] = proj(tx, ty, tz, cam, scale, cx, cy);
      return { x: px, y: py, z: pz };
    });

    const projFaces: PF[] = PENTACHORON.faces.map(fi => {
      const fvs = fi.map(i => projected[i]!);
      const pts = fvs.map(v => `${v.x},${v.y}`).join(' ');
      const depth = fvs.reduce((s, v) => s + v.z, 0) / fvs.length;
      const facing = faceNZ(fvs[0]!.x, fvs[0]!.y, fvs[1]!.x, fvs[1]!.y, fvs[2]!.x, fvs[2]!.y);
      return { points: pts, depth, facing };
    });
    projFaces.sort((a, b) => a.depth - b.depth);

    const projEdges: PE[] = PENTACHORON.edges.map(([a, b]) => {
      const pa = projected[a]!, pb = projected[b]!;
      return { x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y, depth: (pa.z + pb.z) / 2 };
    });
    projEdges.sort((a, b) => a.depth - b.depth);

    setPVerts(projected);
    setPEdges(projEdges);
    setPFaces(projFaces);
  }, [speed1, speed2]);

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

  const depthBright = (z: number) => 0.2 + 0.8 * ((z + 1.5) / 3);
  const edgeStroke = (z: number) => 2.0 * (0.5 + 0.5 * depthBright(z));
  const faceOpacity = (d: number, f: number) => {
    const base = 0.02 + 0.04 * depthBright(d);
    return f > 0 ? base * 1.4 : base * 0.4;
  };

  return (
    <div className={className} style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <filter id={`p5-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2.5" result="blur" />
            <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
          </filter>
        </defs>
        <g>{pFaces.map((f, i) => <polygon key={`f${i}`} points={f.points} fill={glowColor} opacity={faceOpacity(f.depth, f.facing)} />)}</g>
        <g opacity={0.15} filter={`url(#p5-${filterId})`}>
          {pEdges.map((e, i) => <line key={`g${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2} stroke={glowColor} strokeWidth={edgeStroke(e.depth) + 1.5} strokeLinecap="round" opacity={depthBright(e.depth)} />)}
        </g>
        <g>{pEdges.map((e, i) => <line key={`e${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2} stroke={color} strokeWidth={edgeStroke(e.depth)} strokeLinecap="round" opacity={depthBright(e.depth)} />)}</g>
        <g>{[...pVerts].map((v, i) => ({ ...v, i })).sort((a, b) => a.z - b.z).map(v => {
          const b = depthBright(v.z);
          return <circle key={`v${v.i}`} cx={v.x} cy={v.y} r={3.2 * (0.5 + 0.5 * b)} fill={glowColor} opacity={b} />;
        })}</g>
      </svg>
    </div>
  );
}

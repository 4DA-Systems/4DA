// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { COMPOUND_FIVE } from './geometries';

type PE = { x1: number; y1: number; x2: number; y2: number; depth: number; color: string };
type PF = { points: string; depth: number; facing: number; color: string };
type PV = { x: number; y: number; z: number; color: string; i: number };

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

interface CompoundFiveSVGProps {
  size?: number;
  className?: string;
}

/**
 * Compound of five tetrahedra — polychrome BrandMark-quality rendering.
 * 5 interlocking tetrahedra with distinct gold tints, depth-sorted globally.
 */
export function CompoundFiveSVG({ size = 180, className }: CompoundFiveSVGProps) {
  const filterId = useId().replace(/:/g, '');
  const frameRef = useRef(0);
  const rafRef = useRef(0);

  const [allEdges, setAllEdges] = useState<PE[]>([]);
  const [allFaces, setAllFaces] = useState<PF[]>([]);
  const [allVerts, setAllVerts] = useState<PV[]>([]);

  const computeFrame = useCallback(() => {
    const f = frameRef.current;
    const scale = 32;
    const cam = 5.5;
    const cx = 50, cy = 50;
    const ay = f * 0.009;
    const tiltX = 0.32 + Math.sin(f * 0.0022) * 0.1;

    const edges: PE[] = [];
    const faces: PF[] = [];
    const verts: PV[] = [];
    let vi = 0;

    for (const group of COMPOUND_FIVE.groups) {
      const projected = group.vertices.map(([vx, vy, vz]) => {
        const [rx, ry, rz] = rotY(vx, vy, vz, ay);
        const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
        const [px, py, pz] = proj(tx, ty, tz, cam, scale, cx, cy);
        return { x: px, y: py, z: pz };
      });

      for (const fi of group.faces) {
        const fvs = fi.map(i => projected[i]!);
        const pts = fvs.map(v => `${v.x},${v.y}`).join(' ');
        const depth = fvs.reduce((s, v) => s + v.z, 0) / fvs.length;
        const facing = faceNZ(fvs[0]!.x, fvs[0]!.y, fvs[1]!.x, fvs[1]!.y, fvs[2]!.x, fvs[2]!.y);
        faces.push({ points: pts, depth, facing, color: group.color });
      }

      for (const [a, b] of group.edges) {
        const pa = projected[a]!, pb = projected[b]!;
        edges.push({ x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y, depth: (pa.z + pb.z) / 2, color: group.color });
      }

      for (const p of projected) {
        verts.push({ ...p, color: group.color, i: vi++ });
      }
    }

    faces.sort((a, b) => a.depth - b.depth);
    edges.sort((a, b) => a.depth - b.depth);

    setAllEdges(edges);
    setAllFaces(faces);
    setAllVerts(verts);
  }, []);

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
  const edgeStroke = (z: number) => 1.8 * (0.5 + 0.5 * depthBright(z));
  const faceOpacity = (d: number, f: number) => {
    const base = 0.02 + 0.03 * depthBright(d);
    return f > 0 ? base * 1.3 : base * 0.3;
  };

  return (
    <div className={className} style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <filter id={`c5-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="2" result="blur" />
            <feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge>
          </filter>
        </defs>
        <g>{allFaces.map((f, i) => <polygon key={`f${i}`} points={f.points} fill={f.color} opacity={faceOpacity(f.depth, f.facing)} />)}</g>
        <g opacity={0.12} filter={`url(#c5-${filterId})`}>
          {allEdges.map((e, i) => <line key={`g${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2} stroke={e.color} strokeWidth={edgeStroke(e.depth) + 1.2} strokeLinecap="round" opacity={depthBright(e.depth)} />)}
        </g>
        <g>{allEdges.map((e, i) => <line key={`e${i}`} x1={e.x1} y1={e.y1} x2={e.x2} y2={e.y2} stroke={e.color} strokeWidth={edgeStroke(e.depth)} strokeLinecap="round" opacity={depthBright(e.depth)} />)}</g>
        <g>{[...allVerts].sort((a, b) => a.z - b.z).map(v => {
          const b = depthBright(v.z);
          return <circle key={`v${v.i}`} cx={v.x} cy={v.y} r={2.8 * (0.5 + 0.5 * b)} fill={v.color} opacity={b} />;
        })}</g>
      </svg>
    </div>
  );
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Brand-mark geometry: projects the rotating tetrahedron and writes the result
 * straight onto fixed SVG slots. The animation loop calls `drawBrandMark` per
 * frame — no React state, no reconciliation, only the attributes that move.
 */
import {
  TETRA_VERTS,
  TETRA_EDGES,
  TETRA_FACES,
  rotY,
  rotX,
  project,
  faceNormalZ,
} from './math3d';

export type ProjVert = { x: number; y: number; z: number };
export type ProjEdge = { x1: number; y1: number; x2: number; y2: number; depth: number };
export type ProjFace = { points: string; depth: number; facing: number };

export interface BrandMarkGeometry {
  verts: ProjVert[];
  edges: ProjEdge[];
  faces: ProjFace[];
}

/** The SVG groups whose children are the fixed geometry slots. */
export interface BrandMarkSlots {
  faces: SVGGElement | null;
  glow: SVGGElement | null;
  edges: SVGGElement | null;
  verts: SVGGElement | null;
}

/** Rotation speeds are expressed per 30fps frame; this is that frame's length. */
export const BASE_FRAME_MS = 1000 / 30;

export const FACE_COUNT = TETRA_FACES.length;
export const EDGE_COUNT = TETRA_EDGES.length;
export const VERT_COUNT = TETRA_VERTS.length;

const SCALE = 37;
const CAM_DIST = 4.8;
const CENTER = 50;

/** Project the tetrahedron at a rotation angle and secondary-motion frame. */
export function computeGeometry(angleY: number, frame: number): BrandMarkGeometry {
  // Compound rotation: primary Y + slow drifting X tilt (organic, not mechanical).
  const tiltX = 0.35 + Math.sin(frame * 0.0026) * 0.09;
  const verts: ProjVert[] = TETRA_VERTS.map(([vx, vy, vz]) => {
    const [rx, ry, rz] = rotY(vx, vy, vz, angleY);
    const [tx, ty, tz] = rotX(rx, ry, rz, tiltX);
    const [px, py, pz] = project(tx, ty, tz, CAM_DIST, SCALE, CENTER, CENTER);
    return { x: px, y: py, z: pz };
  });

  // Indices are constants 0-3, always in range.
  const faces: ProjFace[] = TETRA_FACES.map(([a, b, c]) => {
    const va = verts[a]!;
    const vb = verts[b]!;
    const vc = verts[c]!;
    return {
      points: `${va.x},${va.y} ${vb.x},${vb.y} ${vc.x},${vc.y}`,
      depth: (va.z + vb.z + vc.z) / 3,
      facing: faceNormalZ(va.x, va.y, vb.x, vb.y, vc.x, vc.y),
    };
  }).sort((a, b) => a.depth - b.depth);

  const edges: ProjEdge[] = TETRA_EDGES.map(([a, b]) => {
    const pa = verts[a]!;
    const pb = verts[b]!;
    return { x1: pa.x, y1: pa.y, x2: pb.x, y2: pb.y, depth: (pa.z + pb.z) / 2 };
  }).sort((a, b) => a.depth - b.depth);

  return { verts, edges, faces };
}

/** Depth brightness: 0.25 (far) to 1.0 (near). */
export const depthBright = (z: number) => 0.25 + 0.75 * ((z + 1.2) / 2.4);

function baseStroke(size: number) {
  return size <= 24 ? 3.5 : size <= 48 ? 2.8 : 2;
}

function baseVertex(size: number) {
  return size <= 24 ? 5.5 : size <= 48 ? 4.5 : 3.5;
}

function faceOpacity(depth: number, facing: number) {
  const base = 0.06 + 0.08 * depthBright(depth);
  return facing > 0 ? base * 1.6 : base * 0.6;
}

const r2 = (n: number) => n.toFixed(2);

function setLine(el: Element | undefined, e: ProjEdge, width: number) {
  if (!el) return;
  el.setAttribute('x1', r2(e.x1));
  el.setAttribute('y1', r2(e.y1));
  el.setAttribute('x2', r2(e.x2));
  el.setAttribute('y2', r2(e.y2));
  el.setAttribute('stroke-width', r2(width));
  el.setAttribute('opacity', r2(depthBright(e.depth)));
}

/**
 * Write one frame onto the slots. Slot order is paint order, so writing the
 * depth-sorted geometry into slots 0..n keeps back-to-front painting correct.
 */
export function drawBrandMark(slots: BrandMarkSlots, geom: BrandMarkGeometry, size: number) {
  const stroke = baseStroke(size);
  const edgeStroke = (z: number) => stroke * (0.6 + 0.4 * depthBright(z));

  geom.faces.forEach((f, i) => {
    const el = slots.faces?.children[i];
    if (!el) return;
    el.setAttribute('points', f.points);
    el.setAttribute('opacity', faceOpacity(f.depth, f.facing).toFixed(3));
  });
  geom.edges.forEach((e, i) => {
    setLine(slots.glow?.children[i], e, edgeStroke(e.depth) + 1.5);
    setLine(slots.edges?.children[i], e, edgeStroke(e.depth));
  });

  const vtx = baseVertex(size);
  [...geom.verts]
    .sort((a, b) => a.z - b.z)
    .forEach((v, i) => {
      const el = slots.verts?.children[i];
      if (!el) return;
      const b = depthBright(v.z);
      el.setAttribute('cx', r2(v.x));
      el.setAttribute('cy', r2(v.y));
      el.setAttribute('r', r2(vtx * (0.6 + 0.4 * b)));
      el.setAttribute('opacity', r2(b));
    });
}

/**
 * Frame interval for a mark of `size` px turning at `rotSpeed` rad per 30fps
 * frame. The rotation is slow (one turn per 18-90 s), so on a 36 px header
 * mark a vertex moves well under a pixel per 30fps frame. Step only as often
 * as needed for ~0.25 px of movement, between 8 and 30 fps.
 */
export function frameIntervalMs(size: number, rotSpeed: number): number {
  const radiusPx = (SCALE / 100) * size;
  const pxPerSecond = Math.abs(rotSpeed) * (1000 / BASE_FRAME_MS) * radiusPx;
  const fps = Math.min(30, Math.max(8, pxPerSecond / 0.25));
  return 1000 / fps;
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Brand-mark geometry: projects the rotating tetrahedron ONCE into a fixed set
 * of frames. The mark is drawn as a sprite sheet of those frames and a CSS
 * `steps()` animation moves the sheet with `transform` — compositor work only,
 * no JavaScript and no re-rasterisation per frame (audit 2026-10-07).
 *
 * The regular tetrahedron has 3-fold symmetry about its vertical axis (apex on
 * +Y, base vertices 120 degrees apart), so one third of a turn is a complete
 * loop: frame FRAME_COUNT is pixel-identical to frame 0.
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

/** Rotation speeds are expressed per 30fps frame; this is that frame's length. */
export const BASE_FRAME_MS = 1000 / 30;

export const FACE_COUNT = TETRA_FACES.length;
export const EDGE_COUNT = TETRA_EDGES.length;
export const VERT_COUNT = TETRA_VERTS.length;

/** One loop of the sprite is a third of a turn (the tetrahedron's symmetry). */
export const LOOP_ANGLE = (2 * Math.PI) / 3;
/** Sprite grid: 12 x 6 = 72 frames per third-turn (1.67 degrees per step). */
export const SPRITE_COLS = 12;
export const SPRITE_ROWS = 6;
export const FRAME_COUNT = SPRITE_COLS * SPRITE_ROWS;

const SCALE = 37;
const CAM_DIST = 4.8;
const CENTER = 50;
/** Fixed X tilt — the centre of the old slow drift (0.26-0.44 rad). */
const TILT_X = 0.35;

/** Project the tetrahedron at a Y rotation angle. */
export function computeGeometry(angleY: number, tiltX = TILT_X): BrandMarkGeometry {
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
      points: `${r2(va.x)},${r2(va.y)} ${r2(vb.x)},${r2(vb.y)} ${r2(vc.x)},${r2(vc.y)}`,
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

const r2 = (n: number) => Number(n.toFixed(2));

export interface LineAttrs {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  strokeWidth: number;
  opacity: number;
}
export interface FaceAttrs {
  points: string;
  opacity: number;
}
export interface VertAttrs {
  cx: number;
  cy: number;
  r: number;
  opacity: number;
}

/** Everything one sprite frame draws, already in paint (back-to-front) order. */
export interface BrandMarkFrame {
  faces: FaceAttrs[];
  glow: LineAttrs[];
  edges: LineAttrs[];
  verts: VertAttrs[];
}

function baseStroke(size: number) {
  return size <= 24 ? 3.5 : size <= 48 ? 2.8 : 2;
}

function baseVertex(size: number) {
  return size <= 24 ? 5.5 : size <= 48 ? 4.5 : 3.5;
}

/** The stroke/vertex scale only changes at these size bins. */
export function sizeBin(size: number): number {
  return size <= 24 ? 24 : size <= 48 ? 48 : 100;
}

function faceOpacity(depth: number, facing: number) {
  const base = 0.06 + 0.08 * depthBright(depth);
  return facing > 0 ? base * 1.6 : base * 0.6;
}

function lineAttrs(e: ProjEdge, width: number): LineAttrs {
  return {
    x1: r2(e.x1),
    y1: r2(e.y1),
    x2: r2(e.x2),
    y2: r2(e.y2),
    strokeWidth: r2(width),
    opacity: r2(depthBright(e.depth)),
  };
}

/** Attributes for one frame of a mark drawn at `size` px. */
export function frameAttrs(geom: BrandMarkGeometry, size: number): BrandMarkFrame {
  const stroke = baseStroke(size);
  const edgeStroke = (z: number) => stroke * (0.6 + 0.4 * depthBright(z));
  const vtx = baseVertex(size);
  return {
    faces: geom.faces.map((f) => ({
      points: f.points,
      opacity: Number(faceOpacity(f.depth, f.facing).toFixed(3)),
    })),
    glow: geom.edges.map((e) => lineAttrs(e, edgeStroke(e.depth) + 1.5)),
    edges: geom.edges.map((e) => lineAttrs(e, edgeStroke(e.depth))),
    verts: [...geom.verts]
      .sort((a, b) => a.z - b.z)
      .map((v) => {
        const b = depthBright(v.z);
        return { cx: r2(v.x), cy: r2(v.y), r: r2(vtx * (0.6 + 0.4 * b)), opacity: r2(b) };
      }),
  };
}

const frameCache = new Map<number, BrandMarkFrame[]>();

/** All FRAME_COUNT frames of one loop for a size bin — computed once, cached. */
export function spriteFrames(size: number): BrandMarkFrame[] {
  const bin = sizeBin(size);
  let frames = frameCache.get(bin);
  if (!frames) {
    frames = Array.from({ length: FRAME_COUNT }, (_, i) =>
      frameAttrs(computeGeometry((i * LOOP_ANGLE) / FRAME_COUNT), bin),
    );
    frameCache.set(bin, frames);
  }
  return frames;
}

/**
 * Duration of one sprite loop (a third of a turn) for a rotation speed in rad
 * per 30fps frame. Rounded to 100 ms so the CSS duration is a stable string.
 */
export function loopDurationMs(rotSpeed: number): number {
  const speed = Math.abs(rotSpeed) || 0.001;
  return Math.round(((LOOP_ANGLE / speed) * BASE_FRAME_MS) / 100) * 100;
}

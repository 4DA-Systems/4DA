// SPDX-License-Identifier: FSL-1.1-Apache-2.0

// ---------------------------------------------------------------------------
// 3D tetrahedron geometry
// ---------------------------------------------------------------------------

/** Regular tetrahedron inscribed in the unit sphere (vertices on r=1) */
export const TETRA_VERTS: [number, number, number][] = [
  [0, 1, 0], // apex
  [0.9428, -0.3333, 0], // front-right
  [-0.4714, -0.3333, 0.8165], // back-left
  [-0.4714, -0.3333, -0.8165], // back-right
];

/** All 6 edges (complete graph K4) */
export const TETRA_EDGES: [number, number][] = [
  [0, 1],
  [0, 2],
  [0, 3],
  [1, 2],
  [1, 3],
  [2, 3],
];

/** 4 triangular faces (vertex index triples, wound consistently) */
export const TETRA_FACES: [number, number, number][] = [
  [0, 1, 2], // apex-front-left
  [0, 2, 3], // apex-left-back
  [0, 3, 1], // apex-back-front
  [1, 3, 2], // base
];

// ---------------------------------------------------------------------------
// 3D math
// ---------------------------------------------------------------------------

export function rotY(
  x: number,
  y: number,
  z: number,
  a: number,
): [number, number, number] {
  const c = Math.cos(a),
    s = Math.sin(a);
  return [c * x + s * z, y, -s * x + c * z];
}

export function rotX(
  x: number,
  y: number,
  z: number,
  a: number,
): [number, number, number] {
  const c = Math.cos(a),
    s = Math.sin(a);
  return [x, c * y - s * z, s * y + c * z];
}

export function project(
  x: number,
  y: number,
  z: number,
  camDist: number,
  scale: number,
  cx: number,
  cy: number,
): [number, number, number] {
  const w = camDist / (camDist - z);
  return [cx + x * scale * w, cy - y * scale * w, z];
}

/** Face normal Z-component (positive = facing camera). Used for backface test. */
export function faceNormalZ(
  ax: number,
  ay: number,
  bx: number,
  by: number,
  cx: number,
  cy: number,
): number {
  return (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
}

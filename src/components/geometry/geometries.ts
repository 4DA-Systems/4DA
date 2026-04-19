// SPDX-License-Identifier: FSL-1.1-Apache-2.0
export interface PolyhedronGeometry {
  vertices: [number, number, number][];
  edges: [number, number][];
  faces: number[][];
}

export interface Polyhedron4DGeometry {
  vertices: [number, number, number, number][];
  edges: [number, number][];
  faces: number[][];
}

// Regular tetrahedron inscribed in unit sphere
export const TETRAHEDRON: PolyhedronGeometry = {
  vertices: [
    [0, 1, 0],
    [0.9428, -0.3333, 0],
    [-0.4714, -0.3333, 0.8165],
    [-0.4714, -0.3333, -0.8165],
  ],
  edges: [
    [0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3],
  ],
  faces: [
    [0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2],
  ],
};

// Regular pentachoron (4-simplex / 5-cell) — 5 vertices in 4D
// Complete graph K5: every vertex connects to every other.
const S5 = 1 / Math.sqrt(5);
export const PENTACHORON: Polyhedron4DGeometry = {
  vertices: [
    [1, 1, 1, -S5],
    [1, -1, -1, -S5],
    [-1, 1, -1, -S5],
    [-1, -1, 1, -S5],
    [0, 0, 0, 4 * S5],
  ],
  edges: [
    [0, 1], [0, 2], [0, 3], [0, 4],
    [1, 2], [1, 3], [1, 4],
    [2, 3], [2, 4],
    [3, 4],
  ],
  faces: [
    [0, 1, 2], [0, 1, 3], [0, 1, 4],
    [0, 2, 3], [0, 2, 4], [0, 3, 4],
    [1, 2, 3], [1, 2, 4], [1, 3, 4],
    [2, 3, 4],
  ],
};

// Regular icosahedron — 12 vertices, 30 edges, 20 triangular faces
// Vertices from golden-ratio rectangles, normalized to unit sphere.
const PHI = (1 + Math.sqrt(5)) / 2;
const ICO_NORM = Math.sqrt(1 + PHI * PHI);
function icoV(a: number, b: number, c: number): [number, number, number] {
  return [a / ICO_NORM, b / ICO_NORM, c / ICO_NORM];
}
export const ICOSAHEDRON: PolyhedronGeometry = {
  vertices: [
    icoV(0, 1, PHI), icoV(0, -1, PHI), icoV(0, 1, -PHI), icoV(0, -1, -PHI),
    icoV(1, PHI, 0), icoV(-1, PHI, 0), icoV(1, -PHI, 0), icoV(-1, -PHI, 0),
    icoV(PHI, 0, 1), icoV(-PHI, 0, 1), icoV(PHI, 0, -1), icoV(-PHI, 0, -1),
  ],
  edges: [
    [0, 1], [0, 4], [0, 5], [0, 8], [0, 9],
    [1, 6], [1, 7], [1, 8], [1, 9],
    [2, 3], [2, 4], [2, 5], [2, 10], [2, 11],
    [3, 6], [3, 7], [3, 10], [3, 11],
    [4, 5], [4, 8], [4, 10],
    [5, 9], [5, 11],
    [6, 7], [6, 8], [6, 10],
    [7, 9], [7, 11],
    [8, 10], [9, 11],
  ],
  faces: [
    [0, 1, 8], [0, 1, 9], [0, 4, 8], [0, 4, 5], [0, 5, 9],
    [1, 6, 8], [1, 6, 7], [1, 7, 9],
    [2, 3, 10], [2, 3, 11], [2, 4, 10], [2, 4, 5], [2, 5, 11],
    [3, 6, 10], [3, 6, 7], [3, 7, 11],
    [4, 8, 10], [6, 8, 10],
    [5, 9, 11], [7, 9, 11],
  ],
};

// Regular dodecahedron — 20 vertices, 30 edges, 12 pentagonal faces
// Built from cube vertices + golden-ratio rectangle vertices.
const P = 1 / PHI;
const DOD_RAW: [number, number, number][] = [
  // Cube vertices (8)
  [1, 1, 1], [1, 1, -1], [1, -1, 1], [1, -1, -1],
  [-1, 1, 1], [-1, 1, -1], [-1, -1, 1], [-1, -1, -1],
  // Rectangle vertices XY plane (4)
  [0, PHI, P], [0, PHI, -P], [0, -PHI, P], [0, -PHI, -P],
  // Rectangle vertices YZ plane (4)
  [P, 0, PHI], [P, 0, -PHI], [-P, 0, PHI], [-P, 0, -PHI],
  // Rectangle vertices XZ plane (4)
  [PHI, P, 0], [PHI, -P, 0], [-PHI, P, 0], [-PHI, -P, 0],
];
const DOD_NORM_F = Math.sqrt(3);
export const DODECAHEDRON: PolyhedronGeometry = {
  vertices: DOD_RAW.map(([x, y, z]) => [x / DOD_NORM_F, y / DOD_NORM_F, z / DOD_NORM_F] as [number, number, number]),
  edges: [
    [0, 8], [0, 12], [0, 16],
    [1, 9], [1, 13], [1, 16],
    [2, 10], [2, 12], [2, 17],
    [3, 11], [3, 13], [3, 17],
    [4, 8], [4, 14], [4, 18],
    [5, 9], [5, 15], [5, 18],
    [6, 10], [6, 14], [6, 19],
    [7, 11], [7, 15], [7, 19],
    [8, 9], [10, 11], [12, 14], [13, 15], [16, 17], [18, 19],
  ],
  faces: [
    [0, 8, 4, 14, 12], [0, 12, 2, 17, 16], [0, 16, 1, 9, 8],
    [1, 13, 3, 17, 16], [1, 9, 5, 15, 13],
    [2, 10, 6, 19, 18], [2, 12, 14, 6, 10], [2, 17, 3, 11, 10],
    [3, 13, 15, 7, 11], [4, 8, 9, 5, 18],
    [5, 18, 19, 7, 15], [6, 14, 4, 18, 19],
  ],
};

// Compound of five tetrahedra — 5 tetrahedra sharing the dodecahedron's 20 vertices
// Each tetrahedron is assigned a color group index.
export interface CompoundGeometry {
  groups: { vertices: [number, number, number][]; edges: [number, number][]; faces: number[][]; color: string }[];
}

// 5 tetrahedra inscribed in a dodecahedron. Vertex indices into DODECAHEDRON.vertices.
const C5T_GROUPS: number[][] = [
  [0, 3, 5, 6], [0, 5, 7, 2], [0, 7, 3, 4],
  [1, 2, 6, 5], [1, 4, 3, 6],
];
const C5T_COLORS = ['#D4AF37', '#DAB94A', '#BF8C20', '#B8782A', '#A68538'];
export const COMPOUND_FIVE: CompoundGeometry = {
  groups: C5T_GROUPS.map((indices, gi) => {
    const verts = indices.map(i => DODECAHEDRON.vertices[i]!);
    return {
      vertices: verts,
      edges: [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]] as [number, number][],
      faces: [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]],
      color: C5T_COLORS[gi]!,
    };
  }),
};

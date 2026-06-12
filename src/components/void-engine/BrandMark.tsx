// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import {
  useMemo,
  useId,
  useEffect,
  useRef,
  useState,
  useCallback,
} from "react";
import type { VoidSignal } from "../../types";
import {
  TETRA_VERTS,
  TETRA_EDGES,
  TETRA_FACES,
  rotY,
  rotX,
  project,
  faceNormalZ,
} from "./math3d";
import { deriveSignalVisuals } from "./signal-visuals";
import { useTheme } from "../../lib/theme";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface BrandMarkProps {
  signal?: VoidSignal;
  size?: number;
}

type ProjVert = { x: number; y: number; z: number };
type ProjEdge = {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  depth: number;
};
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
  const filterId = useId().replace(/:/g, "");
  const { isLight } = useTheme();

  // Derive visual state from signal (theme-aware: ink gold on paper)
  const {
    glowOpacity,
    edgeColor,
    vertexColor,
    faceColor,
    stateLabel,
    rotSpeed,
  } = useMemo(() => deriveSignalVisuals(signal, isLight), [signal, isLight]);

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
        x1: pa.x,
        y1: pa.y,
        x2: pb.x,
        y2: pb.y,
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
    const base = 0.06 + 0.08 * depthBright(depth);
    return facing > 0 ? base * 1.6 : base * 0.6;
  };

  // Ambient breathing cycle — keeps the mark alive even in idle state
  const [breathPhase, setBreathPhase] = useState(0);
  useEffect(() => {
    let frame = 0;
    const interval = setInterval(() => {
      frame += 1;
      setBreathPhase(frame);
    }, 100); // 10fps is enough for a slow breath
    return () => clearInterval(interval);
  }, []);

  // Breath oscillates between 0 and 1 on a 4-second cycle
  const breath = (Math.sin(breathPhase * 0.157) + 1) / 2; // 0.157 ≈ 2π / 40 steps = 4s at 10fps
  const breathScale = 1 + breath * 0.04; // 1.0 → 1.04
  const breathGlow = glowOpacity + breath * 0.18; // adds up to 0.18 to base glow

  const showLabel = size >= 100;

  const itemCount = signal?.item_count ?? 0;
  const openWindows = signal?.open_windows ?? 0;

  const titleParts = [`4DA: ${stateLabel}`];
  if (itemCount > 0) titleParts.push(`${itemCount} items`);
  if (openWindows > 0)
    titleParts.push(
      `${openWindows} decision window${openWindows > 1 ? "s" : ""}`,
    );

  const ariaLabel = `4DA status: ${stateLabel}${itemCount > 0 ? `, ${itemCount} items found` : ""}`;

  return (
    <div
      className="brand-mark-container"
      role="status"
      aria-live="polite"
      title={titleParts.join(" \u00b7 ")}
      aria-label={ariaLabel}
      style={{
        width: size,
        height: size,
        position: "relative",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <svg
        width={size}
        height={size}
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style={{
          display: "block",
          transform: `scale(${breathScale})`,
          transition: "transform 0.3s ease",
        }}
      >
        <defs>
          <filter
            id={`glow-${filterId}`}
            x="-50%"
            y="-50%"
            width="200%"
            height="200%"
          >
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

        {/* Edge glow layer — breathes with ambient pulse */}
        <g opacity={breathGlow} filter={`url(#glow-${filterId})`}>
          {edges.map((e, i) => (
            <line
              key={`g${i}`}
              x1={e.x1}
              y1={e.y1}
              x2={e.x2}
              y2={e.y2}
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
              x1={e.x1}
              y1={e.y1}
              x2={e.x2}
              y2={e.y2}
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
            position: "absolute",
            bottom: 8,
            fontSize: 10,
            color:
              (signal?.error ?? 0) > 0.5 || (signal?.critical_count ?? 0) > 0
                ? "var(--color-error)"
                : (signal?.signal_color_shift ?? 0) > 0.5
                  ? "var(--color-accent-gold)"
                  : (signal?.signal_color_shift ?? 0) < -0.3
                    ? (isLight ? "#1D4ED8" : "#4A90D9")
                    : "var(--color-text-muted)",
            letterSpacing: "0.1em",
            textTransform: "uppercase",
            fontFamily: "JetBrains Mono, monospace",
            opacity: 0.6,
            transition: "color 0.3s ease",
          }}
        >
          {stateLabel}
        </span>
      )}
    </div>
  );
}

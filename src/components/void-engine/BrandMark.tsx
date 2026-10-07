// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, useId, useEffect, useLayoutEffect, useRef } from "react";
import type { CSSProperties } from "react";
import type { VoidSignal } from "../../types";
import {
  BASE_FRAME_MS,
  EDGE_COUNT,
  FACE_COUNT,
  VERT_COUNT,
  computeGeometry,
  drawBrandMark,
  frameIntervalMs,
} from "./brand-mark-geometry";
import type { BrandMarkSlots } from "./brand-mark-geometry";
import { deriveSignalVisuals } from "./signal-visuals";
import { useTheme } from "../../lib/theme";
import { isMotionAllowed, subscribeMotionGate } from "../../lib/motion-gate";

interface BrandMarkProps {
  signal?: VoidSignal;
  size?: number;
}

const range = (n: number) => Array.from({ length: n }, (_, i) => i);

/**
 * 4DA brand mark — 3D rotating tetrahedron.
 *
 * 4 vertices, 6 edges, 4 faces. Real 3D geometry with compound rotation,
 * perspective projection, depth-sorted face fills, and depth-scaled edges.
 * Signal-responsive: color, glow, and rotation speed.
 *
 * Cost discipline (audit 2026-10-07: an idle header mark drove ~2,500 DOM
 * mutations/s through three setState calls per frame): the loop writes
 * attributes onto fixed SVG slots through refs — React renders only the
 * structure and colours. It steps only as often as the slow rotation needs
 * (8-30 fps), stops while the window is hidden, and never starts under
 * prefers-reduced-motion (a static frame is drawn instead). The ambient
 * breath is a CSS animation (App.css `brand-mark-breathe`), not a timer.
 */
export function BrandMark({ signal, size = 36 }: BrandMarkProps) {
  const filterId = useId().replace(/:/g, "");
  const { isLight } = useTheme();

  const { glowOpacity, edgeColor, vertexColor, faceColor, stateLabel, rotSpeed } =
    useMemo(() => deriveSignalVisuals(signal, isLight), [signal, isLight]);

  const angleYRef = useRef(0);
  const frameRef = useRef(0); // secondary-motion clock, in 30fps frames
  const speedRef = useRef(rotSpeed);
  speedRef.current = rotSpeed;
  const sizeRef = useRef(size);
  sizeRef.current = size;

  const facesRef = useRef<SVGGElement>(null);
  const glowRef = useRef<SVGGElement>(null);
  const edgesRef = useRef<SVGGElement>(null);
  const vertsRef = useRef<SVGGElement>(null);

  const draw = () => {
    const slots: BrandMarkSlots = {
      faces: facesRef.current,
      glow: glowRef.current,
      edges: edgesRef.current,
      verts: vertsRef.current,
    };
    drawBrandMark(slots, computeGeometry(angleYRef.current, frameRef.current), sizeRef.current);
  };
  const drawRef = useRef(draw);
  drawRef.current = draw;

  // Static frame on mount and whenever the size (stroke/vertex scale) changes.
  useLayoutEffect(() => {
    drawRef.current();
  }, [size]);

  // Animation loop — gated on visibility + reduced motion.
  useEffect(() => {
    let raf = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let running = false;
    let last = 0;

    const tick = (time: number) => {
      raf = 0;
      if (!running) return;
      // Time-based so the rotation speed is independent of the step rate.
      const steps = last === 0 ? 1 : Math.min(time - last, 250) / BASE_FRAME_MS;
      last = time;
      angleYRef.current += speedRef.current * steps;
      frameRef.current += steps;
      drawRef.current();
      timer = setTimeout(() => {
        timer = undefined;
        if (running) raf = requestAnimationFrame(tick);
      }, frameIntervalMs(sizeRef.current, speedRef.current));
    };
    const start = () => {
      if (running) return;
      running = true;
      last = 0;
      raf = requestAnimationFrame(tick);
    };
    const stop = () => {
      running = false;
      if (raf) cancelAnimationFrame(raf);
      if (timer !== undefined) clearTimeout(timer);
      raf = 0;
      timer = undefined;
    };
    const sync = () => (isMotionAllowed() ? start() : stop());

    sync();
    const unsubscribe = subscribeMotionGate(sync);
    return () => {
      unsubscribe();
      stop();
    };
  }, []);

  const showLabel = size >= 100;
  const itemCount = signal?.item_count ?? 0;
  const openWindows = signal?.open_windows ?? 0;

  const titleParts = [`4DA: ${stateLabel}`];
  if (itemCount > 0) titleParts.push(`${itemCount} items`);
  if (openWindows > 0)
    titleParts.push(`${openWindows} decision window${openWindows > 1 ? "s" : ""}`);

  const ariaLabel = `4DA status: ${stateLabel}${itemCount > 0 ? `, ${itemCount} items found` : ""}`;

  const glowStyle = { "--bm-glow": glowOpacity.toFixed(3) } as CSSProperties;

  return (
    <div
      className="brand-mark-container"
      role="status"
      aria-live="polite"
      title={titleParts.join(" · ")}
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
        className="brand-mark-svg"
        width={size}
        height={size}
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        style={{ display: "block" }}
      >
        <defs>
          <filter id={`glow-${filterId}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Face fills — semi-transparent, painted back-to-front. Gives mass. */}
        <g ref={facesRef} data-slot="faces">
          {range(FACE_COUNT).map((i) => (
            <polygon key={`f${i}`} fill={faceColor} />
          ))}
        </g>

        {/* Edge glow layer — breathes with the ambient CSS pulse */}
        <g
          ref={glowRef}
          data-slot="glow"
          className="brand-mark-glow"
          style={glowStyle}
          filter={`url(#glow-${filterId})`}
        >
          {range(EDGE_COUNT).map((i) => (
            <line key={`g${i}`} stroke={vertexColor} strokeLinecap="round" />
          ))}
        </g>

        {/* Sharp edge layer — depth-sorted, width + brightness by depth */}
        <g ref={edgesRef} data-slot="edges">
          {range(EDGE_COUNT).map((i) => (
            <line key={`e${i}`} stroke={edgeColor} strokeLinecap="round" />
          ))}
        </g>

        {/* Vertex dots — near vertices draw on top, sized by depth */}
        <g ref={vertsRef} data-slot="verts">
          {range(VERT_COUNT).map((i) => (
            <circle key={`v${i}`} fill={vertexColor} />
          ))}
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

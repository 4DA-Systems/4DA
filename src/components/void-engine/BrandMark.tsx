// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, useId, useSyncExternalStore } from "react";
import type { CSSProperties } from "react";
import type { VoidSignal } from "../../types";
import { loopDurationMs } from "./brand-mark-geometry";
import { BrandMarkSprite } from "./BrandMarkSprite";
import { BRAND_MARK_CSS } from "./brand-mark-css";
import { deriveSignalVisuals } from "./signal-visuals";
import { useTheme } from "../../lib/theme";
import { isMotionAllowed, subscribeMotionGate } from "../../lib/motion-gate";

interface BrandMarkProps {
  signal?: VoidSignal;
  size?: number;
}

/**
 * 4DA brand mark — 3D rotating tetrahedron.
 *
 * 4 vertices, 6 edges, 4 faces. Real 3D geometry with perspective projection,
 * depth-sorted face fills, and depth-scaled edges. Signal-responsive: colour,
 * glow, and rotation speed.
 *
 * Cost discipline (audit 2026-10-07): the mark used to be redrawn from
 * JavaScript — first three setState calls per frame, then ~100 SVG attribute
 * writes per frame through refs, and every frame re-rasterised a blurred SVG.
 * Now the geometry is projected ONCE into a sprite sheet of one third of a
 * turn (the tetrahedron's symmetry makes that a full loop) and the rotation
 * is a CSS steps() animation of the sheet's transform; the breath is a
 * transform/opacity animation. All of it runs on the compositor: zero
 * JavaScript per frame, and the glow is rasterised once.
 *
 * - Speed: the signal sets the loop duration through `--bm-loop`.
 * - Hidden window: the motion gate sets `data-motion="paused"` (an event, not
 *   a loop) and CSS pauses every animation.
 * - prefers-reduced-motion: CSS drops the animations; the first frame shows.
 */
export function BrandMark({ signal, size = 36 }: BrandMarkProps) {
  const filterId = `bm-glow-${useId().replace(/:/g, "")}`;
  const { isLight } = useTheme();
  const motionAllowed = useSyncExternalStore(subscribeMotionGate, isMotionAllowed, () => false);

  const { glowOpacity, edgeColor, vertexColor, faceColor, stateLabel, rotSpeed } =
    useMemo(() => deriveSignalVisuals(signal, isLight), [signal, isLight]);

  const showLabel = size >= 100;
  const itemCount = signal?.item_count ?? 0;
  const openWindows = signal?.open_windows ?? 0;

  const titleParts = [`4DA: ${stateLabel}`];
  if (itemCount > 0) titleParts.push(`${itemCount} items`);
  if (openWindows > 0)
    titleParts.push(`${openWindows} decision window${openWindows > 1 ? "s" : ""}`);

  const ariaLabel = `4DA status: ${stateLabel}${itemCount > 0 ? `, ${itemCount} items found` : ""}`;

  const markVars = {
    "--bm-glow": glowOpacity.toFixed(3),
    "--bm-edge": edgeColor,
    "--bm-vertex": vertexColor,
    "--bm-face": faceColor,
    "--bm-loop": `${loopDurationMs(rotSpeed)}ms`,
  } as CSSProperties;

  return (
    <div
      className="brand-mark-container"
      role="status"
      aria-live="polite"
      title={titleParts.join(" · ")}
      aria-label={ariaLabel}
      data-motion={motionAllowed ? "running" : "paused"}
      style={{
        ...markVars,
        width: size,
        height: size,
        position: "relative",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {/* React 19 hoists this into <head> once, however many marks render. */}
      <style href="fourda-brand-mark" precedence="default">
        {BRAND_MARK_CSS}
      </style>
      <div className="brand-mark-viewport" style={{ width: size, height: size }}>
        <BrandMarkSprite size={size} filterId={filterId} />
      </div>

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

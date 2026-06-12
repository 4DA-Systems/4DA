// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { VoidSignal } from "../../types";

export interface SignalVisualState {
  glowOpacity: number;
  edgeColor: string;
  vertexColor: string;
  faceColor: string;
  stateLabel: string;
  rotSpeed: number;
}

// Per-theme geometry palettes. SVG presentation attributes can't resolve
// var(), so the visuals are concrete hexes chosen per theme: additive light
// gold for the void, merch-derived ink gold for paper (see App.css tokens).
interface GeomColors { edge: string; vertex: string; face: string }
interface ThemePalette {
  gold: GeomColors;
  error: GeomColors;
  breaking: GeomColors;
  learning: GeomColors;
  idleGlow: number;
}

const PALETTE: { dark: ThemePalette; light: ThemePalette } = {
  dark: {
    gold: { edge: "#C8B560", vertex: "#D4AF37", face: "#D4AF37" },
    error: { edge: "#EF4444", vertex: "#F87171", face: "#EF4444" },
    breaking: { edge: "#F59E0B", vertex: "#FBBF24", face: "#F59E0B" },
    learning: { edge: "#6B93C0", vertex: "#7BA7D4", face: "#6B93C0" },
    idleGlow: 0.25,
  },
  light: {
    gold: { edge: "#8F7118", vertex: "#A8861D", face: "#A8861D" },
    error: { edge: "#DC2626", vertex: "#B91C1C", face: "#DC2626" },
    breaking: { edge: "#B45309", vertex: "#D97706", face: "#B45309" },
    learning: { edge: "#2563EB", vertex: "#3B82F6", face: "#2563EB" },
    idleGlow: 0.12,
  },
};

/** Derive visual state (colors, glow, label, speed) from the current VoidSignal. */
export function deriveSignalVisuals(
  signal: VoidSignal | undefined,
  isLight = false,
): SignalVisualState {
  const p = isLight ? PALETTE.light : PALETTE.dark;
  // Glow is additive light: it carries the dark theme; on paper it reads as
  // smudge, so it runs at roughly half strength.
  const glowScale = isLight ? 0.5 : 1;

  if (!signal) {
    return {
      glowOpacity: p.idleGlow,
      edgeColor: p.gold.edge,
      vertexColor: p.gold.vertex,
      faceColor: p.gold.face,
      stateLabel: "Idle",
      rotSpeed: 0.014,
    };
  }

  const glow =
    (signal.error > 0.5
      ? 0.15
      : 0.25 + signal.heat * 0.2 + signal.pulse * 0.15 + signal.burst * 0.25) * glowScale;

  let { edge, vertex, face } = p.gold;
  if (signal.error > 0.5 || signal.critical_count > 0) {
    ({ edge, vertex, face } = p.error);
  } else if (signal.signal_color_shift > 0.5) {
    ({ edge, vertex, face } = p.breaking);
  } else if (signal.signal_color_shift < -0.3) {
    ({ edge, vertex, face } = p.learning);
  }

  let label = "Idle";
  if (signal.critical_count > 0 && signal.signal_intensity > 0.75) {
    label =
      signal.critical_count > 1 ? `${signal.critical_count} Alerts` : "Alert";
  } else if (signal.signal_color_shift > 0.5) {
    label = "Breaking";
  } else if (signal.signal_color_shift > 0.2) {
    label = "Discovery";
  } else if (signal.signal_color_shift < -0.3) {
    label = "Learning";
  } else if (signal.morph > 0.3) {
    label = "Context";
  } else if (signal.signal_urgency > 0.6) {
    label = "Urgent";
  } else if (signal.item_count === 0 && signal.heat === 0) {
    label = signal.staleness > 0.9 ? "Dormant" : "Awakening";
  } else if (signal.error > 0.5) {
    label = "Error";
  } else if (signal.staleness > 0.8) {
    label = "Stale";
  } else if (signal.pulse > 0.5) {
    label = "Scanning";
  } else if (signal.heat > 0.5) {
    label = "Discoveries";
  } else if (signal.item_count > 0) {
    label = "Active";
  }

  let speed = (2 * Math.PI) / (60 * 30);
  if (signal.error > 0.5) {
    speed = (2 * Math.PI) / (60 * 30);
  } else if (signal.pulse > 0.5) {
    speed = (2 * Math.PI) / (18 * 30);
  } else if (signal.heat > 0.3 || signal.signal_intensity > 0.4) {
    speed = (2 * Math.PI) / (24 * 30);
  } else if (signal.item_count > 0) {
    speed = (2 * Math.PI) / (36 * 30);
  } else if (signal.staleness > 0.9) {
    speed = (2 * Math.PI) / (90 * 30);
  }

  return {
    glowOpacity: glow,
    edgeColor: edge,
    vertexColor: vertex,
    faceColor: face,
    stateLabel: label,
    rotSpeed: speed,
  };
}

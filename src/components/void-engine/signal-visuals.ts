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

const IDLE_STATE: SignalVisualState = {
  glowOpacity: 0.25,
  edgeColor: "#C8B560",
  vertexColor: "#D4AF37",
  faceColor: "#D4AF37",
  stateLabel: "Idle",
  rotSpeed: 0.014,
};

/** Derive visual state (colors, glow, label, speed) from the current VoidSignal. */
export function deriveSignalVisuals(
  signal: VoidSignal | undefined,
): SignalVisualState {
  if (!signal) return IDLE_STATE;

  const glow =
    signal.error > 0.5
      ? 0.15
      : 0.25 + signal.heat * 0.2 + signal.pulse * 0.15 + signal.burst * 0.25;

  let edge = "#C8B560";
  let vertex = "#D4AF37";
  let face = "#D4AF37";
  if (signal.error > 0.5 || signal.critical_count > 0) {
    edge = "#EF4444";
    vertex = "#F87171";
    face = "#EF4444";
  } else if (signal.signal_color_shift > 0.5) {
    edge = "#F59E0B";
    vertex = "#FBBF24";
    face = "#F59E0B";
  } else if (signal.signal_color_shift < -0.3) {
    edge = "#6B93C0";
    vertex = "#7BA7D4";
    face = "#6B93C0";
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

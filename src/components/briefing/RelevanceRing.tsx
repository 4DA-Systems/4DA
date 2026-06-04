// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';

interface RelevanceRingProps {
  /** Overall relevance score, 0–1. Drives the arc sweep. */
  relevance: number;
  /** Confidence, 0–1. Drives the inner core's opacity (subtle per-item variation). */
  confidence: number;
  /** Extra classes for the wrapper svg (e.g. margin/alignment utilities). */
  className?: string;
}

const SIZE = 20;
const CENTER = SIZE / 2;
const RADIUS = 7;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
const clamp01 = (n: number) => Math.max(0, Math.min(1, n));

/**
 * Crisp, deterministic relevance indicator.
 *
 * Replaces the old WebGPU `fourda-score-fingerprint` orb: pure SVG, no GPU
 * context, no animation loop, and it renders identically under
 * `prefers-reduced-motion`. Colour is inherited via `currentColor` from the
 * surrounding tier class (gold / green / secondary / muted), so the ring always
 * matches its "Core / Strong / Match / Faint" label.
 *
 * The arc sweep encodes overall relevance; the inner core's opacity encodes
 * confidence — two legible dimensions instead of four illegible ones at 20px.
 */
export const RelevanceRing = memo(function RelevanceRing({
  relevance,
  confidence,
  className = '',
}: RelevanceRingProps) {
  const arc = CIRCUMFERENCE * clamp01(relevance);
  const coreOpacity = 0.3 + 0.6 * clamp01(confidence);

  return (
    <svg
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      className={`w-5 h-5 flex-shrink-0 ${className}`}
      fill="none"
      aria-hidden="true"
    >
      {/* Track */}
      <circle
        cx={CENTER}
        cy={CENTER}
        r={RADIUS}
        stroke="currentColor"
        strokeOpacity={0.18}
        strokeWidth={2}
      />
      {/* Relevance arc — begins at 12 o'clock, sweeps clockwise */}
      <circle
        cx={CENTER}
        cy={CENTER}
        r={RADIUS}
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeDasharray={`${arc} ${CIRCUMFERENCE}`}
        transform={`rotate(-90 ${CENTER} ${CENTER})`}
      />
      {/* Confidence core */}
      <circle cx={CENTER} cy={CENTER} r={2.25} fill="currentColor" fillOpacity={coreOpacity} />
    </svg>
  );
});

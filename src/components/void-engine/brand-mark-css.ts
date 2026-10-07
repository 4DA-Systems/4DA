// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Brand-mark stylesheet (audit 2026-10-07, wave 1b). The tetrahedron is drawn
 * once as a SPRITE_COLS x SPRITE_ROWS sprite sheet of one third of a turn
 * (brand-mark-geometry.ts) and the rotation is a steps animation of that
 * sheet's transform. Every animated property is transform or opacity on its
 * own layer, so the compositor runs the mark with zero JavaScript and no
 * re-rasterisation of the blurred glow per frame.
 *
 * Kept in TS so the turn keyframes are generated from the same grid constants
 * as the sprite (they cannot drift apart) and so tests can read the rules.
 * BrandMark renders it as a React 19 hoisted, de-duplicated <style>.
 */
import { FRAME_COUNT, SPRITE_COLS, SPRITE_ROWS } from './brand-mark-geometry';

const pct = (v: number) => `${+v.toFixed(4)}%`;

/** One keyframe stop per sprite cell, row-major; 100% wraps to cell 0. */
export function turnKeyframes(): string {
  const stops: string[] = [];
  for (let i = 0; i < FRAME_COUNT; i++) {
    const col = i % SPRITE_COLS;
    const row = Math.floor(i / SPRITE_COLS);
    const x = col ? `-${pct((col * 100) / SPRITE_COLS)}` : '0';
    const y = row ? `-${pct((row * 100) / SPRITE_ROWS)}` : '0';
    stops.push(`  ${pct((i * 100) / FRAME_COUNT)} { transform: translate(${x}, ${y}); }`);
  }
  stops.push('  100% { transform: translate(0, 0); }');
  return `@keyframes brand-mark-turn {\n${stops.join('\n')}\n}`;
}

const STATIC_RULES = `
.brand-mark-viewport {
  position: relative;
  overflow: hidden;
  transform-origin: center;
  will-change: transform;
  animation: brand-mark-breathe 2s ease-in-out infinite alternate;
}
.brand-mark-sprite {
  position: absolute;
  top: 0;
  left: 0;
  will-change: transform;
  animation: brand-mark-turn var(--bm-loop, 20s) steps(1, end) infinite;
}
.brand-mark-sheet {
  position: absolute;
  top: 0;
  left: 0;
  display: block;
}
.brand-mark-sheet .bm-face { fill: var(--bm-face); }
.brand-mark-sheet .bm-edge { stroke: var(--bm-edge); }
.brand-mark-sheet .bm-glow-line { stroke: var(--bm-vertex); }
.brand-mark-sheet .bm-vert { fill: var(--bm-vertex); }
.brand-mark-glow {
  opacity: var(--bm-glow, 0.25);
  will-change: opacity;
  animation: brand-mark-breathe-glow 2s ease-in-out infinite alternate;
}
/* Hidden window (tray-resident) — the motion gate flips this attribute. */
.brand-mark-container[data-motion="paused"] .brand-mark-viewport,
.brand-mark-container[data-motion="paused"] .brand-mark-sprite,
.brand-mark-container[data-motion="paused"] .brand-mark-glow {
  animation-play-state: paused;
}
/* Reduced motion: a static first frame, no breath. */
@media (prefers-reduced-motion: reduce) {
  .brand-mark-viewport,
  .brand-mark-sprite,
  .brand-mark-glow {
    animation-name: none !important;
  }
}
@keyframes brand-mark-breathe {
  from { transform: scale(1); }
  to { transform: scale(1.04); }
}
@keyframes brand-mark-breathe-glow {
  from { opacity: var(--bm-glow, 0.25); }
  to { opacity: calc(var(--bm-glow, 0.25) + 0.18); }
}
`;

export const BRAND_MARK_CSS = `${STATIC_RULES}\n${turnKeyframes()}\n`;

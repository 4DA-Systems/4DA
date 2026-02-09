// Void Engine color palette
// Maps signal states to visual colors

// Core colors for the heartbeat glow
export const VOID_COLORS = {
  // Idle: cool blue-black, nearly invisible
  idle: { r: 26, g: 26, b: 62 },        // #1a1a3e
  // Active: warm gold (from design system accent)
  active: { r: 212, g: 175, b: 55 },     // #D4AF37
  // Error: brief red flicker
  error: { r: 239, g: 68, b: 68 },       // #EF4444
  // Stale: dim gray
  stale: { r: 42, g: 42, b: 42 },        // #2A2A2A
  // Cool: deep blue (learning signals)
  cool: { r: 26, g: 58, b: 110 },        // #1a3a6e
  // Alert: red (critical signals)
  alert: { r: 239, g: 68, b: 68 },       // #EF4444
} as const;

/** Lerp between two RGB colors based on t (0-1) */
export function lerpColor(
  a: { r: number; g: number; b: number },
  b: { r: number; g: number; b: number },
  t: number,
): string {
  const clamp = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
  const r = clamp(a.r + (b.r - a.r) * t);
  const g = clamp(a.g + (b.g - a.g) * t);
  const bl = clamp(a.b + (b.b - a.b) * t);
  return `rgb(${r}, ${g}, ${bl})`;
}

/** Compute the core glow color from signal values */
export function computeCoreColor(
  heat: number,
  error: number,
  staleness: number,
  colorShift: number = 0,
): string {
  if (error > 0.5) {
    return lerpColor(VOID_COLORS.idle, VOID_COLORS.error, error);
  }
  if (staleness > 0.8) {
    return lerpColor(VOID_COLORS.idle, VOID_COLORS.stale, staleness);
  }
  // Base color from heat
  let base = lerpColor(VOID_COLORS.idle, VOID_COLORS.active, heat);

  // Apply signal color shift
  if (colorShift < -0.1) {
    // Cool shift: blend toward deep blue
    const t = Math.min(1, -colorShift);
    base = lerpColor(VOID_COLORS.active, VOID_COLORS.cool, t * heat);
  } else if (colorShift > 0.8) {
    // Hot shift: blend toward red/alert
    const t = (colorShift - 0.8) / 0.2;
    base = lerpColor(VOID_COLORS.active, VOID_COLORS.alert, t * heat);
  }

  return base;
}

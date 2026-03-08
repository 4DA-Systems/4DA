/**
 * Lazy-load and register GAME Web Components.
 *
 * Each component is a self-contained Custom Element (WebGPU + WebGL2 fallback)
 * compiled from .game source by the GAME compiler.
 *
 * Components are loaded on first use to avoid blocking the main bundle.
 */

let registered = false;

const COMPONENTS = {
  'game-celebration-burst': () => import('./game-components/celebration-burst.js'),
  'game-scan-ring': () => import('./game-components/scan-ring.js'),
  'game-status-orb': () => import('./game-components/status-orb.js'),
  'game-boot-ring': () => import('./game-components/boot-ring.js'),
  'game-engagement-bars': () => import('./game-components/engagement-bars.js'),
  'game-achievement-progress': () => import('./game-components/achievement-progress.js'),
  'game-ambient-intelligence': () => import('./game-components/ambient-intelligence.js'),
  'game-score-fingerprint': () => import('./game-components/score-fingerprint.js'),
  'game-decision-countdown': () => import('./game-components/decision-countdown.js'),
  'game-signal-waveform': () => import('./game-components/signal-waveform.js'),
  'game-knowledge-depth': () => import('./game-components/knowledge-depth.js'),
  'game-source-vitals': () => import('./game-components/source-vitals.js'),
  'game-briefing-atmosphere': () => import('./game-components/briefing-atmosphere.js'),
  'game-playbook-pathway': () => import('./game-components/playbook-pathway.js'),
  'game-radar-field': () => import('./game-components/radar-field.js'),
} as const;

export type GameComponentTag = keyof typeof COMPONENTS;

/** Register a single GAME component by tag name. No-op if already registered. */
export async function registerGameComponent(tag: GameComponentTag): Promise<void> {
  if (customElements.get(tag)) return;
  try {
    await COMPONENTS[tag]();
  } catch (err) {
    console.warn(`[GAME] Failed to load ${tag}:`, err);
  }
}

/** Register all GAME components. Call once on app startup for eager loading. */
export async function registerAllGameComponents(): Promise<void> {
  if (registered) return;
  registered = true;
  await Promise.allSettled(
    (Object.keys(COMPONENTS) as GameComponentTag[]).map(registerGameComponent),
  );
}

/**
 * Lazy-load and register GAME Web Components.
 *
 * Each component is a self-contained Custom Element (WebGPU + WebGL2 fallback).
 * The GAME compiler that originally produced these has been retired — these
 * are now standalone, pre-compiled web components with no build-time dependency.
 *
 * Components are loaded on first use to avoid blocking the main bundle.
 */

let registered = false;

const COMPONENTS = {
  'game-celebration-burst': () => import('./game-components/celebration-burst.js'),
  'game-status-orb': () => import('./game-components/status-orb.js'),
  'game-ambient-intelligence': () => import('./game-components/ambient-intelligence.js'),
  'game-score-fingerprint': () => import('./game-components/score-fingerprint.js'),
  'game-decision-countdown': () => import('./game-components/decision-countdown.js'),
  'game-source-vitals': () => import('./game-components/source-vitals.js'),
  'game-briefing-atmosphere': () => import('./game-components/briefing-atmosphere.js'),
  'game-playbook-pathway': () => import('./game-components/playbook-pathway.js'),
  'game-turing-fire': () => import('./game-components/turing-fire.js'),
  'game-tetrahedron': () => import('./game-components/tetrahedron.js'),
  'game-pentachoron': () => import('./game-components/pentachoron.js'),
  'game-icosahedron': () => import('./game-components/icosahedron.js'),
  'game-dodecahedron': () => import('./game-components/dodecahedron.js'),
  'game-compound-five-tetrahedra': () => import('./game-components/compound-five-tetrahedra.js'),
  'game-simplex-unfold': () => import('./game-components/simplex-unfold.js'),
  'game-momentum-field': () => import('./game-components/momentum-field.js'),
  'game-logo-mark': () => import('./game-components/logo-mark.js'),
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

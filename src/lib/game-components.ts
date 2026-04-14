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
  'fourda-celebration-burst': () => import('./game-components/celebration-burst.js'),
  'fourda-status-orb': () => import('./game-components/status-orb.js'),
  'fourda-ambient-intelligence': () => import('./game-components/ambient-intelligence.js'),
  'fourda-score-fingerprint': () => import('./game-components/score-fingerprint.js'),
  'fourda-decision-countdown': () => import('./game-components/decision-countdown.js'),
  'fourda-source-vitals': () => import('./game-components/source-vitals.js'),
  'fourda-briefing-atmosphere': () => import('./game-components/briefing-atmosphere.js'),
  'fourda-playbook-pathway': () => import('./game-components/playbook-pathway.js'),
  'fourda-turing-fire': () => import('./game-components/turing-fire.js'),
  'fourda-tetrahedron': () => import('./game-components/tetrahedron.js'),
  'fourda-pentachoron': () => import('./game-components/pentachoron.js'),
  'fourda-icosahedron': () => import('./game-components/icosahedron.js'),
  'fourda-dodecahedron': () => import('./game-components/dodecahedron.js'),
  'fourda-compound-five-tetrahedra': () => import('./game-components/compound-five-tetrahedra.js'),
  'fourda-simplex-unfold': () => import('./game-components/simplex-unfold.js'),
  'fourda-momentum-field': () => import('./game-components/momentum-field.js'),
  'fourda-logo-mark': () => import('./game-components/logo-mark.js'),
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

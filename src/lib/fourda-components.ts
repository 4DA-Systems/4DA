// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
  'fourda-celebration-burst': () => import('./fourda-components/celebration-burst.js'),
  'fourda-status-orb': () => import('./fourda-components/status-orb.js'),
  'fourda-ambient-intelligence': () => import('./fourda-components/ambient-intelligence.js'),
  'fourda-score-fingerprint': () => import('./fourda-components/score-fingerprint.js'),
  'fourda-decision-countdown': () => import('./fourda-components/decision-countdown.js'),
  'fourda-source-vitals': () => import('./fourda-components/source-vitals.js'),
  'fourda-briefing-atmosphere': () => import('./fourda-components/briefing-atmosphere.js'),
  'fourda-playbook-pathway': () => import('./fourda-components/playbook-pathway.js'),
  'fourda-turing-fire': () => import('./fourda-components/turing-fire.js'),
  'fourda-tetrahedron': () => import('./fourda-components/tetrahedron.js'),
  'fourda-pentachoron': () => import('./fourda-components/pentachoron.js'),
  'fourda-icosahedron': () => import('./fourda-components/icosahedron.js'),
  'fourda-dodecahedron': () => import('./fourda-components/dodecahedron.js'),
  'fourda-compound-five-tetrahedra': () => import('./fourda-components/compound-five-tetrahedra.js'),
  'fourda-simplex-unfold': () => import('./fourda-components/simplex-unfold.js'),
  'fourda-momentum-field': () => import('./fourda-components/momentum-field.js'),
  'fourda-logo-mark': () => import('./fourda-components/logo-mark.js'),
} as const;

export type FourdaComponentTag = keyof typeof COMPONENTS;

/** Register a single GAME component by tag name. No-op if already registered. */
export async function registerFourdaComponent(tag: FourdaComponentTag): Promise<void> {
  if (customElements.get(tag)) return;
  try {
    await COMPONENTS[tag]();
  } catch (err) {
    console.warn(`[GAME] Failed to load ${tag}:`, err);
  }
}

/** Register all GAME components. Call once on app startup for eager loading. */
export async function registerAllFourdaComponents(): Promise<void> {
  if (registered) return;
  registered = true;
  await Promise.allSettled(
    (Object.keys(COMPONENTS) as FourdaComponentTag[]).map(registerFourdaComponent),
  );
}

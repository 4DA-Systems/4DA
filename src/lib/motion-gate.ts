// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Motion gate — one place that answers "may motion run now?".
 *
 * Two tiers:
 * - `isMotionAllowed()` — may a JS animation loop that is converging on a
 *   target run? Not while the window is hidden (the tray-resident app spends
 *   most of its life that way) or when the user asked the OS for reduced
 *   motion. use-void-signals relies on exactly this contract.
 * - `isAmbientMotionAllowed()` — may DECORATIVE, open-ended motion run (the
 *   brand mark's turn and breath)? Additionally requires the window to have
 *   focus: a visible but unfocused window sits on screen for hours, and an
 *   infinite CSS animation there cost the GPU process 25-43% of a core
 *   (measured 2026-10-10).
 *
 * Consumers re-check through `subscribeMotionGate()`, which fires on any
 * input change: visibility, window focus/blur, reduced-motion preference.
 */

const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

function reducedMotionList(): MediaQueryList | null {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return null;
  try {
    return window.matchMedia(REDUCED_MOTION_QUERY);
  } catch {
    return null;
  }
}

/** True when the user prefers reduced motion. */
export function prefersReducedMotion(): boolean {
  return reducedMotionList()?.matches ?? false;
}

/** True when the document is hidden (minimised, tray-hidden, background tab). */
export function isDocumentHidden(): boolean {
  return typeof document !== 'undefined' && document.hidden;
}

/** True when the window has OS focus (the user is looking at / using it). */
export function isWindowFocused(): boolean {
  if (typeof document === 'undefined' || typeof document.hasFocus !== 'function') return true;
  return document.hasFocus();
}

/** True when an animation loop may run: visible document, no reduced-motion preference. */
export function isMotionAllowed(): boolean {
  return !isDocumentHidden() && !prefersReducedMotion();
}

/** True when decorative, open-ended motion may run: `isMotionAllowed()` and a focused window. */
export function isAmbientMotionAllowed(): boolean {
  return isMotionAllowed() && isWindowFocused();
}

/**
 * Call `onChange` whenever visibility, window focus or the reduced-motion
 * preference changes. Returns an unsubscribe function.
 */
export function subscribeMotionGate(onChange: () => void): () => void {
  const mql = reducedMotionList();
  const win = typeof window !== 'undefined' ? window : null;
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', onChange);
  }
  win?.addEventListener('focus', onChange);
  win?.addEventListener('blur', onChange);
  mql?.addEventListener?.('change', onChange);
  return () => {
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', onChange);
    }
    win?.removeEventListener('focus', onChange);
    win?.removeEventListener('blur', onChange);
    mql?.removeEventListener?.('change', onChange);
  };
}

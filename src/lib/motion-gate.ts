// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Motion gate — one place that answers "may a JS animation loop run now?".
 *
 * A requestAnimationFrame loop must not run while the window is hidden (the
 * tray-resident app spends most of its life that way) or when the user asked
 * the OS for reduced motion. Loops call `isMotionAllowed()` before scheduling
 * and re-check through `subscribeMotionGate()` when either input changes.
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

/** True when an animation loop may run: visible document, no reduced-motion preference. */
export function isMotionAllowed(): boolean {
  return !isDocumentHidden() && !prefersReducedMotion();
}

/**
 * Call `onChange` whenever visibility or the reduced-motion preference changes.
 * Returns an unsubscribe function.
 */
export function subscribeMotionGate(onChange: () => void): () => void {
  const mql = reducedMotionList();
  if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', onChange);
  }
  mql?.addEventListener?.('change', onChange);
  return () => {
    if (typeof document !== 'undefined') {
      document.removeEventListener('visibilitychange', onChange);
    }
    mql?.removeEventListener?.('change', onChange);
  };
}

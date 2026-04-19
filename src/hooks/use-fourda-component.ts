// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { registerFourdaComponent, type FourdaComponentTag } from '../lib/fourda-components';

export type FourdaElement = HTMLElement & { setParam?: (name: string, value: number) => void };

/**
 * Detect if the user prefers reduced motion (accessibility).
 * When true, GAME shader animations should not be mounted.
 */
function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Shared hook for mounting a GAME Web Component into a container div.
 * Handles registration, DOM element creation, and cleanup.
 * Respects prefers-reduced-motion — returns reducedMotion flag so callers
 * can show a static fallback instead.
 *
 * Returns containerRef (attach to a div), elementRef (for calling setParam),
 * and reducedMotion (true if animations are disabled).
 */
export function useFourdaComponent(tag: FourdaComponentTag) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const elementRef = useRef<FourdaElement | null>(null);
  const [reducedMotion, setReducedMotion] = useState(prefersReducedMotion);

  // Listen for changes to reduced-motion preference
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const mql = window.matchMedia('(prefers-reduced-motion: reduce)');
    const handler = (e: MediaQueryListEvent) => setReducedMotion(e.matches);
    mql.addEventListener('change', handler);
    return () => mql.removeEventListener('change', handler);
  }, []);

  useEffect(() => {
    // Don't mount GPU animations if user prefers reduced motion
    if (reducedMotion) return;

    registerFourdaComponent(tag).then(() => {
      if (!containerRef.current || elementRef.current) return;
      const el = document.createElement(tag);
      el.style.width = '100%';
      el.style.height = '100%';
      el.style.display = 'block';
      el.setAttribute('aria-hidden', 'true');
      containerRef.current.appendChild(el);
      elementRef.current = el as FourdaElement;
    });
    const container = containerRef.current;
    return () => {
      if (elementRef.current && container?.contains(elementRef.current)) {
        container.removeChild(elementRef.current);
      }
      elementRef.current = null;
    };
  }, [tag, reducedMotion]);

  return { containerRef, elementRef, reducedMotion };
}

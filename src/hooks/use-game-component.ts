import { useEffect, useRef } from 'react';
import { registerGameComponent, type GameComponentTag } from '../lib/game-components';

export type GameElement = HTMLElement & { setParam?: (name: string, value: number) => void };

/**
 * Shared hook for mounting a GAME Web Component into a container div.
 * Handles registration, DOM element creation, and cleanup.
 *
 * Returns containerRef (attach to a div) and elementRef (for calling setParam).
 */
export function useGameComponent(tag: GameComponentTag) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const elementRef = useRef<GameElement | null>(null);

  useEffect(() => {
    registerGameComponent(tag).then(() => {
      if (!containerRef.current || elementRef.current) return;
      const el = document.createElement(tag);
      el.style.width = '100%';
      el.style.height = '100%';
      el.style.display = 'block';
      containerRef.current.appendChild(el);
      elementRef.current = el as GameElement;
    });
    const container = containerRef.current;
    return () => {
      if (elementRef.current && container?.contains(elementRef.current)) {
        container.removeChild(elementRef.current);
      }
      elementRef.current = null;
    };
  }, [tag]);

  return { containerRef, elementRef };
}

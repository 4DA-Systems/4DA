import { forwardRef, useRef, useEffect, useCallback, useImperativeHandle } from 'react';

import type { VoidSignal } from '../../types';
import { registerGameComponent } from '../../lib/game-components';

export interface VoidBannerHandle {
  setCursor: (x: number, y: number) => void;
}

interface VoidBannerProps {
  signal: VoidSignal;
}

/**
 * Full-width GAME shader banner rendered behind the app header.
 *
 * Uses the compiled `<game-intelligence-banner>` GAME component for rendering.
 * Maps VoidSignal properties to shader uniforms via setParam().
 * Exposes a `setCursor(x, y)` imperative method for mouse tracking.
 */
export const VoidBanner = forwardRef<VoidBannerHandle, VoidBannerProps>(
  function VoidBanner({ signal }, ref) {
    const containerRef = useRef<HTMLDivElement>(null);
    const elementRef = useRef<HTMLElement | null>(null);

    // Expose cursor control to parent
    useImperativeHandle(ref, () => ({
      setCursor: (x: number, y: number) => {
        const el = elementRef.current as
          | (HTMLElement & { setParam?: (n: string, v: number) => void })
          | null;
        el?.setParam?.('cursor_x', x);
        el?.setParam?.('cursor_y', y);
      },
    }));

    // Register the GAME component on mount
    useEffect(() => {
      let cancelled = false;
      registerGameComponent('game-intelligence-banner').then(() => {
        if (cancelled || !containerRef.current) return;
        const el = document.createElement('game-intelligence-banner');
        el.style.width = '100%';
        el.style.height = '100%';
        el.style.display = 'block';
        containerRef.current.appendChild(el);
        elementRef.current = el;
      }).catch(() => {
        // GAME component failed — silent fallback
      });
      const container = containerRef.current;
      return () => {
        cancelled = true;
        if (elementRef.current && container?.contains(elementRef.current)) {
          container.removeChild(elementRef.current);
        }
        elementRef.current = null;
      };
    }, []);

    // Map VoidSignal to GAME component params
    const setParam = useCallback((name: string, value: number) => {
      const el = elementRef.current as
        | (HTMLElement & { setParam?: (n: string, v: number) => void })
        | null;
      el?.setParam?.(name, value);
    }, []);

    useEffect(() => {
      setParam('pulse', signal.pulse);
      setParam('heat', signal.heat);
      setParam('burst', signal.burst);
      setParam('morph', signal.morph);
      setParam('error_val', signal.error);
      setParam('staleness', signal.staleness);
      setParam('opacity_val', signal.staleness > 0.8 ? 0.7 : 0.7 + signal.heat * 0.15);
      setParam('signal_intensity', signal.signal_intensity);
      setParam('color_shift', signal.signal_color_shift);
      setParam('critical_count', signal.critical_count);
      setParam('metabolism', signal.metabolism);
    }, [signal, setParam]);

    return (
      <div
        ref={containerRef}
        className="w-full h-full"
        aria-hidden="true"
        style={{ position: 'absolute', inset: 0 }}
      />
    );
  },
);

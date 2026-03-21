import { forwardRef, useRef, useEffect, useCallback, useImperativeHandle, useMemo } from 'react';

import type { VoidSignal } from '../../types';
import { registerGameComponent } from '../../lib/game-components';

export interface VoidBannerHandle {
  setCursor: (x: number, y: number) => void;
}

interface VoidBannerProps {
  signal: VoidSignal;
}

/**
 * Regime-based GAME shader banner — maps app state into distinct visual regimes
 * instead of just passing raw parameters. The banner should honestly reflect
 * what's happening: calm when idle, urgent when signals need attention.
 */
export const VoidBanner = forwardRef<VoidBannerHandle, VoidBannerProps>(
  function VoidBanner({ signal }, ref) {
    const containerRef = useRef<HTMLDivElement>(null);
    const elementRef = useRef<HTMLElement | null>(null);

    useImperativeHandle(ref, () => ({
      setCursor: (x: number, y: number) => {
        const el = elementRef.current as
          | (HTMLElement & { setParam?: (n: string, v: number) => void })
          | null;
        el?.setParam?.('cursor_x', x);
        el?.setParam?.('cursor_y', y);
      },
    }));

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

    const setParam = useCallback((name: string, value: number) => {
      const el = elementRef.current as
        | (HTMLElement & { setParam?: (n: string, v: number) => void })
        | null;
      el?.setParam?.(name, value);
    }, []);

    // Compute visual regime from app state
    const regime = useMemo(() => {
      // Critical alerts: red, urgent, high energy
      if (signal.critical_count > 0 || signal.error > 0.5) {
        return {
          pulse: Math.max(signal.pulse, 0.7),
          heat: Math.max(signal.heat, 0.8),
          burst: Math.max(signal.burst, 0.6),
          morph: 0.4,
          error_val: signal.error,
          staleness: 0,
          opacity: 0.9,
          intensity: 1.0,
          color_shift: 0.8,  // warm/alert
          critical: signal.critical_count,
          metabolism: Math.max(signal.metabolism, 0.6),
        };
      }

      // Active analysis: vibrant, flowing, responsive
      if (signal.pulse > 0.3) {
        return {
          pulse: signal.pulse,
          heat: signal.heat,
          burst: signal.burst,
          morph: signal.morph,
          error_val: signal.error,
          staleness: 0,
          opacity: 0.8 + signal.heat * 0.15,
          intensity: signal.signal_intensity,
          color_shift: signal.signal_color_shift,
          critical: 0,
          metabolism: signal.metabolism,
        };
      }

      // Fresh results: warm afterglow proportional to quality
      if (signal.heat > 0.2 && signal.staleness < 0.3) {
        return {
          pulse: 0.1,
          heat: signal.heat,
          burst: signal.burst,
          morph: signal.morph * 0.5,
          error_val: 0,
          staleness: signal.staleness,
          opacity: 0.7 + signal.heat * 0.15,
          intensity: signal.signal_intensity,
          color_shift: signal.signal_color_shift,
          critical: 0,
          metabolism: signal.metabolism,
        };
      }

      // Idle/stale: honestly dim, minimal movement
      return {
        pulse: 0.02,
        heat: signal.heat * 0.3,
        burst: 0,
        morph: 0.01,
        error_val: 0,
        staleness: signal.staleness,
        opacity: 0.45 + (1 - signal.staleness) * 0.2,
        intensity: 0,
        color_shift: -0.3,  // cool when idle
        critical: 0,
        metabolism: signal.metabolism * 0.5,
      };
    }, [signal]);

    // Apply regime to shader params
    useEffect(() => {
      setParam('pulse', regime.pulse);
      setParam('heat', regime.heat);
      setParam('burst', regime.burst);
      setParam('morph', regime.morph);
      setParam('error_val', regime.error_val);
      setParam('staleness', regime.staleness);
      setParam('opacity_val', regime.opacity);
      setParam('signal_intensity', regime.intensity);
      setParam('color_shift', regime.color_shift);
      setParam('critical_count', regime.critical);
      setParam('metabolism', regime.metabolism);
    }, [regime, setParam]);

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

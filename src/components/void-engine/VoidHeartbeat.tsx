import { useRef, useEffect, useMemo, useCallback } from 'react';
import type { VoidSignal } from '../../types';
import { registerGameComponent } from '../../lib/game-components';

interface VoidHeartbeatProps {
  signal: VoidSignal;
  size?: number;
}

/**
 * Ambient heartbeat indicator for 4DA.
 *
 * Uses the compiled `<game-ambient-intelligence>` GAME component for rendering.
 * Maps VoidSignal properties to shader uniforms via setParam().
 * Falls back to CSS if the GAME component fails to load.
 */
export function VoidHeartbeat({ signal, size = 200 }: VoidHeartbeatProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const elementRef = useRef<HTMLElement | null>(null);

  // Register the GAME component on mount
  useEffect(() => {
    let cancelled = false;
    registerGameComponent('game-ambient-intelligence').then(() => {
      if (cancelled || !containerRef.current) return;
      const el = document.createElement('game-ambient-intelligence');
      el.style.width = '100%';
      el.style.height = '100%';
      el.style.display = 'block';
      containerRef.current.appendChild(el);
      elementRef.current = el;
    }).catch(() => {
      // GAME component failed — CSS fallback already visible
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
    const el = elementRef.current as (HTMLElement & { setParam?: (n: string, v: number) => void }) | null;
    el?.setParam?.(name, value);
  }, []);

  useEffect(() => {
    setParam('pulse', signal.pulse);
    setParam('heat', signal.heat);
    setParam('burst', signal.burst);
    setParam('morph', signal.morph);
    setParam('error_val', signal.error);
    setParam('staleness', signal.staleness);
    setParam('opacity_val', signal.staleness > 0.8 ? 0.9 : 0.9 + signal.heat * 0.1);
    setParam('signal_intensity', signal.signal_intensity);
    setParam('color_shift', signal.signal_color_shift);
    setParam('critical_count', signal.critical_count);
    setParam('metabolism', signal.metabolism);
  }, [signal, setParam]);

  // State label
  const stateLabel = useMemo(() => {
    if (signal.critical_count > 0 && signal.signal_intensity > 0.75) {
      return signal.critical_count > 1 ? `${signal.critical_count} Alerts` : 'Alert';
    }
    if (signal.signal_color_shift > 0.5) return 'Breaking';
    if (signal.signal_color_shift > 0.2) return 'Discovery';
    if (signal.signal_color_shift < -0.3) return 'Learning';
    if (signal.morph > 0.3) return 'Context';
    if (signal.signal_urgency > 0.6) return 'Urgent';
    if (signal.item_count === 0 && signal.heat === 0) {
      return signal.staleness > 0.9 ? 'Dormant' : 'Awakening';
    }
    if (signal.error > 0.5) return 'Error';
    if (signal.staleness > 0.8) return 'Stale';
    if (signal.pulse > 0.5) return 'Scanning';
    if (signal.heat > 0.5) return 'Discoveries';
    if (signal.item_count > 0) return 'Active';
    return 'Idle';
  }, [signal]);

  return (
    <div
      className="void-heartbeat-container"
      role="status"
      aria-live="polite"
      title={`4DA: ${stateLabel}${signal.item_count > 0 ? ` · ${signal.item_count} items` : ''}${signal.open_windows > 0 ? ` · ${signal.open_windows} decision window${signal.open_windows > 1 ? 's' : ''}` : ''}`}
      aria-label={`4DA status: ${stateLabel}${signal.item_count > 0 ? `, ${signal.item_count} items found` : ''}`}
      style={{
        width: size,
        height: size,
        position: 'relative',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      {/* GAME component container */}
      <div
        ref={containerRef}
        style={{
          width: size,
          height: size,
          position: 'absolute',
          top: 0,
          left: 0,
          pointerEvents: 'none',
        }}
      />

      {/* State label */}
      {size >= 100 && (
        <span
          className="void-heartbeat-label"
          style={{
            position: 'absolute',
            bottom: 8,
            fontSize: 10,
            color: signal.error > 0.5 || signal.critical_count > 0 ? 'var(--color-error)'
              : signal.signal_color_shift > 0.5 ? 'var(--color-accent-gold)'
              : signal.signal_color_shift < -0.3 ? '#4A90D9'
              : 'var(--color-text-muted)',
            letterSpacing: '0.1em',
            textTransform: 'uppercase',
            fontFamily: 'JetBrains Mono, monospace',
            opacity: 0.6,
            transition: 'color 0.3s ease',
          }}
        >
          {stateLabel}
        </span>
      )}
    </div>
  );
}

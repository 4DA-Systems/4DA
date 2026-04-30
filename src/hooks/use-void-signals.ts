// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { VoidSignal } from '../types';
import { cmd } from '../lib/commands';

const IDLE_SIGNAL: VoidSignal = {
  pulse: 0,
  heat: 0,
  burst: 0,
  morph: 0,
  error: 0,
  staleness: 1,
  item_count: 0,
  signal_intensity: 0,
  signal_urgency: 0,
  critical_count: 0,
  signal_color_shift: 0,
  metabolism: 0,
  open_windows: 0,
  advantage_trend: 0,
};

/** Lerp a single value toward target */
function lerp(current: number, target: number, speed: number): number {
  const diff = target - current;
  if (Math.abs(diff) < 0.001) return target;
  return current + diff * speed;
}

/**
 * Hook that listens for void-signal events and provides smooth interpolation.
 * Returns the current interpolated signal values, updated at ~30fps.
 */
export function useVoidSignals() {
  const [signal, setSignal] = useState<VoidSignal>(IDLE_SIGNAL);
  const targetRef = useRef<VoidSignal>(IDLE_SIGNAL);
  const currentRef = useRef<VoidSignal>(IDLE_SIGNAL);
  const rafRef = useRef<number>(0);

  // Fetch initial state on mount
  useEffect(() => {
    cmd('get_void_signal')
      .then((s) => {
        targetRef.current = s;
        currentRef.current = s;
        setSignal(s);
      })
      .catch((e) => console.debug('[void-signals] not available:', e));
  }, []);

  // Listen for change events from Rust
  useEffect(() => {
    let cancelled = false;
    const setup = async () => {
      const unlisten = await listen<VoidSignal>('void-signal', (event) => {
        if (!cancelled && event.payload) {
          targetRef.current = event.payload;
        }
      });
      return unlisten;
    };
    const promise = setup();
    return () => {
      cancelled = true;
      promise.then((fn) => fn());
    };
  }, []);

  // Animation loop: interpolate current toward target at ~30fps
  useEffect(() => {
    let lastTime = 0;
    let cancelled = false;
    const FRAME_INTERVAL = 1000 / 30; // 30fps

    const animate = (time: number) => {
      if (cancelled) return;
      rafRef.current = requestAnimationFrame(animate);

      if (time - lastTime < FRAME_INTERVAL) return;
      lastTime = time;

      const target = targetRef.current;
      const current = currentRef.current;
      if (!target || !current) return;
      const speed = 0.08;

      const next: VoidSignal = {
        pulse: lerp(current.pulse, target.pulse, speed),
        heat: lerp(current.heat, target.heat, speed),
        burst: lerp(current.burst, target.burst, speed * 3), // Burst decays faster
        morph: lerp(current.morph, target.morph, speed),
        error: lerp(current.error, target.error, speed * 2), // Error appears/disappears faster
        staleness: lerp(current.staleness, target.staleness, speed * 0.5), // Staleness changes slowly
        item_count: target.item_count, // No interpolation for integer
        signal_intensity: lerp(current.signal_intensity, target.signal_intensity, speed * 2), // Fast response
        signal_urgency: lerp(current.signal_urgency, target.signal_urgency, speed),
        critical_count: target.critical_count, // No interpolation for integer
        signal_color_shift: lerp(current.signal_color_shift, target.signal_color_shift, speed * 1.5),
        metabolism: lerp(current.metabolism, target.metabolism, speed * 0.5), // Slow — tracks calibration health
        open_windows: target.open_windows, // No interpolation for integer
        advantage_trend: lerp(current.advantage_trend, target.advantage_trend, speed),
      };

      // Only trigger re-render if values actually changed visually
      const changed =
        Math.abs(next.pulse - current.pulse) > 0.001 ||
        Math.abs(next.heat - current.heat) > 0.001 ||
        Math.abs(next.burst - current.burst) > 0.001 ||
        Math.abs(next.morph - current.morph) > 0.001 ||
        Math.abs(next.error - current.error) > 0.001 ||
        Math.abs(next.staleness - current.staleness) > 0.001 ||
        next.item_count !== current.item_count ||
        Math.abs(next.signal_intensity - current.signal_intensity) > 0.001 ||
        Math.abs(next.signal_color_shift - current.signal_color_shift) > 0.001 ||
        next.critical_count !== current.critical_count;

      currentRef.current = next;

      if (changed) {
        setSignal({ ...next });
      }
    };

    rafRef.current = requestAnimationFrame(animate);
    return () => {
      cancelled = true;
      cancelAnimationFrame(rafRef.current);
    };
  }, []);

  return signal;
}

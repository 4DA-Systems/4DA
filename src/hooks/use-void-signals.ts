// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useRef } from 'react';
import type { VoidSignal } from '../types';
import { cmd } from '../lib/commands';
import { safeListen } from '../lib/tauri-events';
import { isDocumentHidden, prefersReducedMotion, subscribeMotionGate } from '../lib/motion-gate';

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

const FRAME_INTERVAL = 1000 / 30; // 30fps
const BASE_SPEED = 0.08;

/** Per-field interpolation speed multipliers; integer fields snap. */
const FIELD_SPEED: Partial<Record<keyof VoidSignal, number>> = {
  pulse: 1,
  heat: 1,
  burst: 3, // Burst decays faster
  morph: 1,
  error: 2, // Error appears/disappears faster
  staleness: 0.5, // Staleness changes slowly
  signal_intensity: 2, // Fast response
  signal_urgency: 1,
  signal_color_shift: 1.5,
  metabolism: 0.5, // Slow — tracks calibration health
  advantage_trend: 1,
};

const FIELDS = Object.keys(IDLE_SIGNAL) as (keyof VoidSignal)[];

/** Lerp a single value toward target; snaps once within 0.001. */
function lerp(current: number, target: number, speed: number): number {
  const diff = target - current;
  if (Math.abs(diff) < 0.001) return target;
  return current + diff * speed;
}

/** One interpolation step from `current` toward `target`. */
export function stepVoidSignal(current: VoidSignal, target: VoidSignal): VoidSignal {
  const next = { ...target };
  for (const key of FIELDS) {
    const mult = FIELD_SPEED[key];
    if (mult !== undefined) next[key] = lerp(current[key], target[key], BASE_SPEED * mult);
  }
  return next;
}

/** True when every field of `a` equals `b` — the loop has nothing left to do. */
export function voidSignalConverged(a: VoidSignal, b: VoidSignal): boolean {
  return FIELDS.every((key) => a[key] === b[key]);
}

/**
 * Fill missing / non-finite fields from the idle signal. A NaN field would
 * never converge and would keep the loop alive forever.
 */
export function normalizeVoidSignal(raw: Partial<VoidSignal>): VoidSignal {
  const out = { ...IDLE_SIGNAL };
  for (const key of FIELDS) {
    const v = raw[key];
    if (typeof v === 'number' && Number.isFinite(v)) out[key] = v;
  }
  return out;
}

/** True when the step moved some field by a visible amount. */
function visiblyChanged(a: VoidSignal, b: VoidSignal): boolean {
  return FIELDS.some((key) => Math.abs(a[key] - b[key]) > 0.001);
}

/**
 * Hook that listens for void-signal events and provides smooth interpolation.
 *
 * The rAF loop runs only while the current value is converging on a new
 * target: it stops once current === target, restarts when a 'void-signal'
 * event brings a new target, pauses while the window is hidden, and is never
 * used under prefers-reduced-motion (the target is applied directly).
 * Audit 2026-10-07: the previous loop ran forever at 60 callbacks/s.
 */
export function useVoidSignals() {
  const [signal, setSignal] = useState<VoidSignal>(IDLE_SIGNAL);
  const targetRef = useRef<VoidSignal>(IDLE_SIGNAL);
  const currentRef = useRef<VoidSignal>(IDLE_SIGNAL);
  const renderedRef = useRef<VoidSignal>(IDLE_SIGNAL);
  /** Re-evaluates whether the loop should run; set by the loop effect. */
  const syncRef = useRef<() => void>(() => {});

  // Fetch initial state on mount
  useEffect(() => {
    void cmd('get_void_signal')
      .then((raw) => {
        if (!raw) return;
        const s = normalizeVoidSignal(raw);
        targetRef.current = s;
        currentRef.current = s;
        renderedRef.current = s;
        setSignal(s);
      })
      .catch((e) => console.debug('[void-signals] not available:', e));
  }, []);

  // Listen for change events from Rust
  useEffect(() => {
    let cancelled = false;
    const setup = async () => {
      const unlisten = await safeListen<VoidSignal>('void-signal', (event) => {
        if (!cancelled && event.payload) {
          targetRef.current = normalizeVoidSignal(event.payload);
          syncRef.current();
        }
      });
      return unlisten;
    };
    const promise = setup();
    return () => {
      cancelled = true;
      void promise.then((fn) => fn());
    };
  }, []);

  // Interpolation loop: runs only until current reaches target.
  useEffect(() => {
    let raf = 0;
    let lastTime = 0;
    let running = false;

    const publish = (value: VoidSignal) => {
      renderedRef.current = value;
      setSignal({ ...value });
    };
    const stop = () => {
      running = false;
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    };
    const animate = (time: number) => {
      raf = 0;
      if (!running) return;
      if (time - lastTime < FRAME_INTERVAL) {
        raf = requestAnimationFrame(animate);
        return;
      }
      lastTime = time;
      const target = targetRef.current;
      const next = stepVoidSignal(currentRef.current, target);
      currentRef.current = next;
      const settled = voidSignalConverged(next, target);
      if (visiblyChanged(next, renderedRef.current) || (settled && !voidSignalConverged(next, renderedRef.current))) {
        publish(next);
      }
      if (settled) {
        running = false;
        return;
      }
      raf = requestAnimationFrame(animate);
    };
    const sync = () => {
      const target = targetRef.current;
      if (prefersReducedMotion()) {
        // No interpolation: jump straight to the target, no loop.
        stop();
        currentRef.current = target;
        if (!voidSignalConverged(target, renderedRef.current)) publish(target);
        return;
      }
      if (isDocumentHidden() || voidSignalConverged(currentRef.current, target)) {
        stop();
        return;
      }
      if (running) return;
      running = true;
      lastTime = 0;
      raf = requestAnimationFrame(animate);
    };

    syncRef.current = sync;
    sync();
    const unsubscribe = subscribeMotionGate(sync);
    return () => {
      unsubscribe();
      stop();
      syncRef.current = () => {};
    };
  }, []);

  return signal;
}

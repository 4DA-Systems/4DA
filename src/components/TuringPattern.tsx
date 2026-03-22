import { useEffect, useRef, useCallback, useState } from 'react';
import { initTuringGPU, turingFrame } from './turing-shader-setup';
import type { TuringGPUState } from './turing-shader-setup';

/**
 * GPU-accelerated Turing pattern via WebGPU compute shaders.
 *
 * Gray-Scott reaction-diffusion with gold-on-dark cosine palette.
 * Bilinear sampling, radial vignette, smooth fade-in built into the shader.
 * Properly destroys all GPU resources on unmount.
 */

/* eslint-disable @typescript-eslint/no-explicit-any */

interface TuringPatternProps {
  style?: React.CSSProperties;
  className?: string;
  /** Simulation grid (default 384 — good balance of detail vs perf) */
  gridSize?: number;
  /** Compute iterations per frame (default 6) */
  stepsPerFrame?: number;
  /** Feed rate (default 0.037 — coral-like fingers) */
  feed?: number;
  /** Kill rate (default 0.06) */
  kill?: number;
  /** Called when WebGPU is confirmed available and rendering */
  onReady?: () => void;
  /** Called if WebGPU is unavailable */
  onFallback?: () => void;
}

export function TuringPattern({
  style,
  className,
  gridSize = 384,
  stepsPerFrame = 6,
  feed = 0.037,
  kill = 0.06,
  onReady,
  onFallback,
}: TuringPatternProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const gpuRef = useRef<{ state: TuringGPUState; raf: number } | null>(null);
  const [ready, setReady] = useState(false);

  const cleanup = useCallback(() => {
    const g = gpuRef.current;
    if (g) {
      cancelAnimationFrame(g.raf);
      try { g.state.device.destroy(); } catch { /* already destroyed */ }
      gpuRef.current = null;
    }
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    let cancelled = false;

    (async () => {
      try {
        const gpuState = await initTuringGPU(canvas, gridSize);
        if (cancelled) {
          gpuState?.device.destroy();
          return;
        }
        if (!gpuState) {
          onFallback?.();
          return;
        }

        const ref = { state: gpuState, raf: 0 };
        gpuRef.current = ref;

        let currentA = gpuState.bufA;
        let currentB = gpuState.bufB;
        let frameCount = 0;

        setReady(true);
        onReady?.();

        const frame = () => {
          if (cancelled) return;

          [currentA, currentB] = turingFrame(
            gpuState,
            canvas,
            feed,
            kill,
            stepsPerFrame,
            frameCount,
            currentA,
            currentB,
          );

          frameCount++;
          ref.raf = requestAnimationFrame(frame);
        };

        ref.raf = requestAnimationFrame(frame);
      } catch (err) {
        console.warn('[TuringPattern] WebGPU init failed:', err);
        if (!cancelled) onFallback?.();
      }
    })();

    return () => {
      cancelled = true;
      const g = gpuRef.current;
      if (g?.state.ro) g.state.ro.disconnect();
      cleanup();
    };
  }, [gridSize, stepsPerFrame, feed, kill, cleanup, onReady, onFallback]);

  return (
    <canvas
      ref={canvasRef}
      className={className}
      style={{
        display: 'block',
        width: '100%',
        height: '100%',
        opacity: ready ? 1 : 0,
        transition: 'opacity 500ms ease-in',
        ...style,
      }}
    />
  );
}

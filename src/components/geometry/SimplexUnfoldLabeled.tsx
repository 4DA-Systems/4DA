import { useEffect, useState } from 'react';
import { useGameComponent } from '../../hooks/use-game-component';

const DIMENSION_LABELS = [
  '0D · Point',
  '1D · Line',
  '2D · Triangle',
  '3D · Tetrahedron',
  '4D · Pentachoron',
] as const;

interface SimplexUnfoldLabeledProps {
  size?: number;
  autoSpeed?: number;
  showLabel?: boolean;
}

/**
 * Simplex-unfold with dimension label overlay.
 * Shows "0D Point" → "1D Line" → ... → "4D Pentachoron" as the animation progresses.
 */
export function SimplexUnfoldLabeled({
  size = 200,
  autoSpeed = 0.12,
  showLabel = true,
}: SimplexUnfoldLabeledProps) {
  const { containerRef, elementRef } = useGameComponent('game-simplex-unfold');
  const [phase, setPhase] = useState(0);

  // Track the current phase from the animation's auto-cycle
  useEffect(() => {
    if (!showLabel) return;
    const interval = setInterval(() => {
      const elapsed = (performance.now() / 1000) * autoSpeed / 6.0;
      const raw = (elapsed % 1.0) * 6.0;
      const clamped = Math.min(raw, 5.0);
      setPhase(Math.min(Math.floor(clamped), 4));
    }, 200); // check 5 times/second

    return () => clearInterval(interval);
  }, [autoSpeed, showLabel]);

  useEffect(() => {
    elementRef.current?.setParam?.('auto_speed', autoSpeed);
  }, [autoSpeed, elementRef]);

  return (
    <div style={{ width: size, height: size, position: 'relative' }}>
      <div
        ref={containerRef}
        style={{ width: '100%', height: '100%' }}
      />
      {showLabel && (
        <span
          style={{
            position: 'absolute',
            bottom: Math.max(4, size * 0.04),
            left: 0,
            right: 0,
            textAlign: 'center',
            fontSize: Math.max(9, size * 0.06),
            color: 'var(--color-text-muted, #8A8A8A)',
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
            fontFamily: 'JetBrains Mono, monospace',
            opacity: 0.7,
            transition: 'opacity 0.3s ease',
            pointerEvents: 'none',
          }}
        >
          {DIMENSION_LABELS[phase]}
        </span>
      )}
    </div>
  );
}

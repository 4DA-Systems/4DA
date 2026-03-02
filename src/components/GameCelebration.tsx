import { useEffect, useRef, useState } from 'react';
import { useAppStore } from '../store';
import type { AchievementTier } from '../store/game-slice';
import { getGameIcon } from '../lib/game-icons';
import { registerGameComponent } from '../lib/game-components';

const TIER_COLORS: Record<AchievementTier, string> = {
  gold: '#D4AF37',
  silver: '#C0C0C0',
  bronze: '#CD7F32',
};

/**
 * Full-screen celebration overlay that appears when achievements unlock.
 * GPU-powered ring burst via <game-celebration-burst> with CSS fallback.
 */
export function GameCelebration() {
  const celebration = useAppStore(s => s.celebration);
  const clearCelebration = useAppStore(s => s.clearCelebration);
  const containerRef = useRef<HTMLDivElement>(null);
  const burstRef = useRef<HTMLElement>(null);
  const [gpuReady, setGpuReady] = useState(false);

  // Lazy-register the GPU component once
  useEffect(() => {
    registerGameComponent('game-celebration-burst').then(() => {
      if (customElements.get('game-celebration-burst')) setGpuReady(true);
    });
  }, []);

  // Drive the burst intensity: spike based on tier, decay to 0
  useEffect(() => {
    if (!celebration || !burstRef.current) return;
    const el = burstRef.current as HTMLElement & { intensity?: number };
    const peakIntensity = celebration.celebration_intensity ?? 1.0;
    el.intensity = peakIntensity;

    // Decay over 1.2s to match CSS ring-burst timing
    const start = performance.now();
    const duration = 1200;
    function decay() {
      const elapsed = performance.now() - start;
      const t = Math.min(1, elapsed / duration);
      el.intensity = peakIntensity * (1.0 - t);
      if (t < 1) requestAnimationFrame(decay);
    }
    requestAnimationFrame(decay);
  }, [celebration]);

  if (!celebration) return null;

  const icon = getGameIcon(celebration.icon);

  return (
    <div
      ref={containerRef}
      className="fixed inset-0 z-[100] pointer-events-none flex items-center justify-center"
      aria-live="assertive"
      role="alert"
    >
      {/* GPU ring burst — replaces CSS .game-ring-burst divs */}
      {gpuReady ? (
        <game-celebration-burst
          ref={burstRef}
          style={{
            position: 'absolute',
            width: '320px',
            height: '320px',
            pointerEvents: 'none',
          }}
        />
      ) : (
        <>
          <div className="game-ring-burst" />
          <div className="game-ring-burst game-ring-burst-delayed" />
        </>
      )}

      {/* Achievement card */}
      <div
        className="game-achievement-card pointer-events-auto cursor-pointer"
        onClick={clearCelebration}
      >
        <div className="text-3xl mb-1">{icon}</div>
        <div
          className="text-xs uppercase tracking-widest font-semibold mb-1"
          style={{ color: celebration.tier ? TIER_COLORS[celebration.tier] : '#D4AF37' }}
        >
          {celebration.tier ? `${celebration.tier} ` : ''}Achievement Unlocked
        </div>
        <div className="text-lg font-bold text-white">{celebration.name}</div>
        <div className="text-xs text-gray-400 mt-1">{celebration.description}</div>
      </div>
    </div>
  );
}


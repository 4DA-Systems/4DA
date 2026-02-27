import { useEffect, useRef } from 'react';
import { useAppStore } from '../store';

const ICON_MAP: Record<string, string> = {
  sun: '\u2600',
  eye: '\uD83D\uDC41',
  radar: '\uD83D\uDCE1',
  sparkle: '\u2728',
  gem: '\uD83D\uDC8E',
  bookmark: '\uD83D\uDD16',
  archive: '\uD83D\uDCE6',
  scroll: '\uD83D\uDCDC',
  fire: '\uD83D\uDD25',
  flame: '\uD83D\uDD25',
  crown: '\uD83D\uDC51',
  globe: '\uD83C\uDF10',
  brain: '\uD83E\uDDE0',
};

/**
 * Full-screen celebration overlay that appears when achievements unlock.
 * Renders a golden ring burst + toast notification, then auto-dismisses.
 */
export function GameCelebration() {
  const celebration = useAppStore(s => s.celebration);
  const clearCelebration = useAppStore(s => s.clearCelebration);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!celebration) return;

    // Force reflow to trigger animation
    const el = containerRef.current;
    if (el) {
      void el.offsetHeight; // force reflow
    }
  }, [celebration]);

  if (!celebration) return null;

  const icon = ICON_MAP[celebration.icon] || '\u2B50';

  return (
    <div
      ref={containerRef}
      className="fixed inset-0 z-[100] pointer-events-none flex items-center justify-center"
      aria-live="assertive"
      role="alert"
    >
      {/* Ring burst effect */}
      <div className="game-ring-burst" />
      <div className="game-ring-burst game-ring-burst-delayed" />

      {/* Achievement card */}
      <div
        className="game-achievement-card pointer-events-auto cursor-pointer"
        onClick={clearCelebration}
      >
        <div className="text-3xl mb-1">{icon}</div>
        <div className="text-xs uppercase tracking-widest text-[#D4AF37] font-semibold mb-1">
          Achievement Unlocked
        </div>
        <div className="text-lg font-bold text-white">{celebration.title}</div>
        <div className="text-xs text-gray-400 mt-1">{celebration.description}</div>
      </div>
    </div>
  );
}

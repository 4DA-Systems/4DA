import { useMemo, useState, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { hashString, isInUserStack } from './momentum-utils';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STRIP_HEIGHT = 140;
const NODE_LANES = 5;
const LANE_HEIGHT = STRIP_HEIGHT / NODE_LANES;

function momentumX(entry: RadarEntry): number {
  switch (entry.movement) {
    case 'down':   return 5 + (hashString(entry.name) % 20);
    case 'stable': return 30 + (hashString(entry.name) % 40);
    case 'up':     return 72 + (hashString(entry.name) % 18);
    case 'new':    return 80 + (hashString(entry.name) % 15);
  }
}

function nodeRadius(score: number): number {
  if (score >= 0.85) return 22;
  if (score >= 0.7) return 17;
  if (score >= 0.5) return 13;
  return 10;
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

function PulseTooltip({ name, movement, x, y }: { name: string; movement: string; x: number; y: number }) {
  const { t } = useTranslation();
  const label = movement === 'up' ? t('momentum.rising') : movement === 'down' ? t('momentum.declining')
    : movement === 'new' ? t('momentum.new') : t('momentum.steady');
  return (
    <div
      className="fixed z-50 px-2.5 py-1.5 rounded-md bg-[#1F1F1F] border border-border shadow-lg pointer-events-none"
      style={{ left: x, top: y - 40 }}
    >
      <span className="text-xs text-white font-medium">{name}</span>
      <span className="text-[10px] text-text-muted ml-2">{label}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MomentumPulseProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

export const MomentumPulse = memo(function MomentumPulse({ entries, userStack, onEntryClick }: MomentumPulseProps) {
  const { t } = useTranslation();
  const [tooltip, setTooltip] = useState<{ name: string; movement: string; x: number; y: number } | null>(null);

  const nodes = useMemo(() => {
    const top = [...entries].sort((a, b) => b.score - a.score).slice(0, 24);
    const laneOccupancy = Array.from({ length: NODE_LANES }, () => [] as number[]);

    return top.map(entry => {
      const xPct = momentumX(entry);
      const r = nodeRadius(entry.score);
      const inStack = isInUserStack(entry.name, userStack);

      // Pick the least-crowded lane
      let bestLane = 0;
      let bestCount = Infinity;
      for (let lane = 0; lane < NODE_LANES; lane++) {
        const conflicts = laneOccupancy[lane]!.filter(ox => Math.abs(ox - xPct) < 8).length;
        if (conflicts < bestCount) { bestCount = conflicts; bestLane = lane; }
      }
      laneOccupancy[bestLane]!.push(xPct);

      const yOffset = bestLane * LANE_HEIGHT + LANE_HEIGHT / 2;

      return { entry, xPct, yOffset, r, inStack };
    });
  }, [entries, userStack]);

  const handleMouseEnter = useCallback((e: React.MouseEvent, name: string, movement: string) => {
    const rect = (e.target as HTMLElement).getBoundingClientRect();
    setTooltip({ name, movement, x: rect.left + rect.width / 2, y: rect.top });
  }, []);

  const handleMouseLeave = useCallback(() => setTooltip(null), []);

  return (
    <div className="relative border-b border-border">
      {/* Zone labels */}
      <div className="absolute inset-x-0 top-2 flex justify-between px-6 pointer-events-none select-none">
        <span className="text-[9px] text-red-400/40 uppercase tracking-widest">{t('momentum.declining')}</span>
        <span className="text-[9px] text-text-muted/30 uppercase tracking-widest">{t('momentum.steady')}</span>
        <span className="text-[9px] text-green-400/40 uppercase tracking-widest">{t('momentum.rising')}</span>
      </div>

      {/* Gradient zones */}
      <div className="absolute inset-0 pointer-events-none" style={{
        background: 'linear-gradient(90deg, rgba(239,68,68,0.03) 0%, transparent 30%, transparent 70%, rgba(34,197,94,0.03) 100%)',
      }} />

      {/* Node field */}
      <div className="relative mx-4" style={{ height: STRIP_HEIGHT }}>
        {nodes.map(({ entry, xPct, yOffset, r, inStack }, i) => {
          const mvColor = entry.movement === 'up' ? '#22C55E'
            : entry.movement === 'down' ? '#EF4444'
            : entry.movement === 'new' ? '#D4AF37' : '#555';

          return (
            <button
              key={entry.name}
              onClick={() => onEntryClick(entry)}
              onMouseEnter={e => handleMouseEnter(e, entry.name, entry.movement)}
              onMouseLeave={handleMouseLeave}
              className="absolute rounded-full transition-all duration-500 hover:scale-110 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent-gold"
              style={{
                left: `${xPct}%`,
                top: yOffset - r / 2,
                width: r * 2,
                height: r * 2,
                transform: 'translateX(-50%)',
                backgroundColor: 'rgba(255,255,255,0.85)',
                boxShadow: inStack
                  ? `0 0 0 3px #D4AF37, 0 0 12px ${mvColor}40`
                  : `0 0 8px ${mvColor}30`,
                opacity: 0,
                animation: `fadeIn 0.5s ease-out ${i * 40}ms forwards`,
              }}
              aria-label={`${entry.name} — ${entry.movement}`}
            />
          );
        })}
      </div>

      {tooltip && <PulseTooltip {...tooltip} />}
    </div>
  );
});

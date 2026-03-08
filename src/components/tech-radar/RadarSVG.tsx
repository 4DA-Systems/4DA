import { useState, useEffect, useMemo, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';

export interface RadarEntry {
  name: string;
  ring: 'adopt' | 'trial' | 'assess' | 'hold';
  quadrant: 'languages' | 'frameworks' | 'tools' | 'platforms';
  movement: 'up' | 'down' | 'stable' | 'new';
  signals: string[];
  decision_ref: number | null;
  score: number;
}

export interface RadarSVGProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

const CX = 300;
const CY = 300;
const FULL_VIEWBOX = '0 0 600 600';
const RING_RADII: Record<string, number> = { adopt: 75, trial: 150, assess: 225, hold: 280 };
const RING_LABEL_KEYS = ['techRadar.ringAdopt', 'techRadar.ringTrial', 'techRadar.ringAssess', 'techRadar.ringHold'];
const RING_KEYS = ['adopt', 'trial', 'assess', 'hold'];

const QUADRANT_CONFIG: Record<string, { label: string; startAngle: number; endAngle: number }> = {
  languages:  { label: 'Languages',  startAngle: Math.PI,       endAngle: Math.PI * 1.5 },
  frameworks: { label: 'Frameworks', startAngle: Math.PI * 1.5, endAngle: Math.PI * 2 },
  tools:      { label: 'Tools',      startAngle: Math.PI * 0.5, endAngle: Math.PI },
  platforms:  { label: 'Platforms',   startAngle: 0,             endAngle: Math.PI * 0.5 },
};

type QuadrantKey = 'languages' | 'frameworks' | 'tools' | 'platforms';
const QUADRANT_VIEWBOXES: Record<QuadrantKey, string> = {
  languages: '0 0 300 300', frameworks: '300 0 300 300',
  tools: '0 300 300 300', platforms: '300 300 300 300',
};
const QUAD_LABEL_POS: Record<QuadrantKey, { x: number; y: number }> = {
  languages: { x: 80, y: 30 }, frameworks: { x: 520, y: 30 },
  tools: { x: 80, y: 590 }, platforms: { x: 520, y: 590 },
};

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash) + str.charCodeAt(i);
    hash |= 0;
  }
  return Math.abs(hash);
}

function getEntryPosition(entry: RadarEntry): { x: number; y: number } {
  const ring = RING_RADII[entry.ring];
  const prevRing = entry.ring === 'adopt' ? 0
    : entry.ring === 'trial' ? RING_RADII.adopt
    : entry.ring === 'assess' ? RING_RADII.trial : RING_RADII.assess;
  const quad = QUADRANT_CONFIG[entry.quadrant];
  const h = hashString(entry.name);
  const radius = prevRing + (ring - prevRing) * (((h % 1000) / 1000) * 0.7 + 0.15);
  const angle = quad.startAngle + (quad.endAngle - quad.startAngle) * ((((h >> 10) % 1000) / 1000) * 0.7 + 0.15);
  return { x: CX + radius * Math.cos(angle), y: CY + radius * Math.sin(angle) };
}

function isInUserStack(name: string, stack: string[]): boolean {
  const lower = name.toLowerCase();
  return stack.some(s => s.toLowerCase() === lower);
}

const dotTransition = (idx: number) =>
  `cx 0.6s cubic-bezier(0.34,1.56,0.64,1) ${idx * 30}ms, cy 0.6s cubic-bezier(0.34,1.56,0.64,1) ${idx * 30}ms`;

const MovementIndicator = memo(function MovementIndicator(
  { x, y, movement }: { x: number; y: number; movement: RadarEntry['movement'] },
) {
  const cx = x, cy = y - 9;
  if (movement === 'up') return <polygon points={`${cx},${cy - 4} ${cx - 3},${cy + 2} ${cx + 3},${cy + 2}`} fill="#22C55E" />;
  if (movement === 'down') return <polygon points={`${cx},${cy + 4} ${cx - 3},${cy - 2} ${cx + 3},${cy - 2}`} fill="#EF4444" />;
  if (movement === 'new') return <polygon points={`${cx},${cy - 4} ${cx + 3},${cy} ${cx},${cy + 4} ${cx - 3},${cy}`} fill="#D4AF37" />;
  return null;
});

interface TooltipState { entry: RadarEntry; x: number; y: number }

const RadarTooltip = memo(function RadarTooltip({ tooltip }: { tooltip: TooltipState }) {
  const tx = tooltip.x > 400 ? tooltip.x - 170 : tooltip.x + 15;
  const ty = tooltip.y > 450 ? tooltip.y - 80 : tooltip.y + 15;
  const ringLabel = tooltip.entry.ring.charAt(0).toUpperCase() + tooltip.entry.ring.slice(1);
  const quadLabel = tooltip.entry.quadrant.charAt(0).toUpperCase() + tooltip.entry.quadrant.slice(1);
  return (
    <g>
      <rect x={tx} y={ty} width="160"
        height={56 + Math.min(tooltip.entry.signals.length, 3) * 14}
        rx="6" fill="#1F1F1F" stroke="#2A2A2A" strokeWidth="1" />
      <text x={tx + 8} y={ty + 16} fill="#FFFFFF" fontSize="11" fontWeight="600">{tooltip.entry.name}</text>
      <text x={tx + 8} y={ty + 30} fill="#A0A0A0" fontSize="9">{ringLabel} / {quadLabel}</text>
      <text x={tx + 8} y={ty + 44} fill="#666666" fontSize="9">
        Score: {tooltip.entry.score.toFixed(2)} | Movement: {tooltip.entry.movement}
      </text>
      {tooltip.entry.signals.slice(0, 3).map((signal, i) => (
        <text key={i} x={tx + 8} y={ty + 58 + i * 14} fill="#666666" fontSize="8">
          - {signal.length > 28 ? signal.slice(0, 28) + '...' : signal}
        </text>
      ))}
    </g>
  );
});

export const RadarSVG = memo(function RadarSVG({ entries, userStack, onEntryClick }: RadarSVGProps) {
  const { t } = useTranslation();
  const [zoomedQuadrant, setZoomedQuadrant] = useState<QuadrantKey | null>(null);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setMounted(true), 50);
    return () => clearTimeout(timer);
  }, []);

  const positionedEntries = useMemo(() =>
    entries.map((entry) => ({ entry, pos: getEntryPosition(entry) })),
  [entries]);

  const handleQuadrantClick = useCallback((q: QuadrantKey) => {
    setZoomedQuadrant(prev => prev === q ? null : q);
    setTooltip(null);
  }, []);
  const handleBackClick = useCallback(() => { setZoomedQuadrant(null); setTooltip(null); }, []);
  const handleMouseEnter = useCallback((entry: RadarEntry, x: number, y: number) => setTooltip({ entry, x, y }), []);
  const handleMouseLeave = useCallback(() => setTooltip(null), []);
  const handleDotClick = useCallback((entry: RadarEntry) => onEntryClick(entry), [onEntryClick]);

  const isZoomed = zoomedQuadrant !== null;
  const currentViewBox = isZoomed ? QUADRANT_VIEWBOXES[zoomedQuadrant] : FULL_VIEWBOX;

  return (
    <div className="relative">
      <svg
        viewBox={currentViewBox}
        role="img"
        aria-label={t('techRadar.svgLabel')}
        className="w-full max-w-[560px]"
        style={{ fontFamily: 'Inter, sans-serif', transition: 'all 0.4s cubic-bezier(0.4, 0, 0.2, 1)' }}
      >
        <rect x="0" y="0" width="600" height="600" fill="transparent" rx="8" />
        {isZoomed && (
          <rect x="0" y="0" width="600" height="600" fill="transparent"
            onClick={handleBackClick} style={{ cursor: 'pointer' }} />
        )}

        {/* Crosshair lines */}
        <line x1={CX} y1={CX - 290} x2={CX} y2={CY + 290} stroke="#2A2A2A" strokeWidth="1" />
        <line x1={CX - 290} y1={CY} x2={CX + 290} y2={CY} stroke="#2A2A2A" strokeWidth="1" />

        {/* Concentric rings */}
        {RING_KEYS.map((ring) => (
          <circle key={ring} cx={CX} cy={CY} r={RING_RADII[ring]}
            fill="none" stroke="#2A2A2A" strokeWidth="1" />
        ))}

        {/* Ring labels */}
        {RING_KEYS.map((ring, i) => {
          const prevR = i === 0 ? 0 : RING_RADII[RING_KEYS[i - 1]];
          return (
            <text key={ring} x={CX + (prevR + RING_RADII[ring]) / 2} y={CY - 6}
              textAnchor="middle" fill="#666666" fontSize="9" fontWeight="500">
              {t(RING_LABEL_KEYS[i])}
            </text>
          );
        })}

        {/* Clickable quadrant labels */}
        {(Object.keys(QUAD_LABEL_POS) as QuadrantKey[]).map((quad) => (
          <text key={quad} x={QUAD_LABEL_POS[quad].x} y={QUAD_LABEL_POS[quad].y}
            textAnchor="middle" fill={zoomedQuadrant === quad ? '#D4AF37' : '#A0A0A0'}
            fontSize="11" fontWeight="600"
            onClick={(e) => { e.stopPropagation(); handleQuadrantClick(quad); }}
            style={{ cursor: 'pointer', transition: 'fill 0.2s' }}>
            {QUADRANT_CONFIG[quad].label}
          </text>
        ))}

        {/* Entry dots */}
        {positionedEntries.map(({ entry, pos }, index) => {
          const inStack = isInUserStack(entry.name, userStack);
          const isGlowing = entry.movement === 'up' || entry.movement === 'new';
          const r = isZoomed ? 7 : 5;
          const cx = mounted ? pos.x : CX;
          const cy = mounted ? pos.y : CY;
          const transition = dotTransition(index);
          return (
            <g key={entry.name}
              role="button"
              aria-label={`${entry.name} — ${entry.ring}, ${entry.quadrant}, ${entry.movement}`}
              tabIndex={0}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); handleDotClick(entry); } }}
              onMouseEnter={() => handleMouseEnter(entry, pos.x, pos.y)}
              onMouseLeave={handleMouseLeave}
              onClick={(e) => { e.stopPropagation(); handleDotClick(entry); }}
              style={{ cursor: 'pointer' }}
              className={isGlowing ? 'radar-glow' : undefined}>
              <circle cx={cx} cy={cy} r={r} fill="#FFFFFF" opacity="0.9" style={{ transition }} />
              {inStack && (
                <circle cx={cx} cy={cy} r={r + 3} fill="none"
                  stroke="#D4AF37" strokeWidth="2" opacity="0.8" style={{ transition }} />
              )}
              <circle cx={cx} cy={cy} r={r + 5} fill="transparent" style={{ transition }} />
              {mounted && entry.movement !== 'stable' && (
                <MovementIndicator x={pos.x} y={pos.y} movement={entry.movement} />
              )}
              {isZoomed && mounted && (
                <text x={pos.x + r + 4} y={pos.y + 3} fill="#A0A0A0" fontSize="8"
                  fontWeight="500" style={{ pointerEvents: 'none' }}>
                  {entry.name.length > 12 ? entry.name.slice(0, 11) + '\u2026' : entry.name}
                </text>
              )}
            </g>
          );
        })}

        {tooltip && !isZoomed && <RadarTooltip tooltip={tooltip} />}
      </svg>

      {isZoomed && (
        <button onClick={handleBackClick}
          className="absolute top-2 left-2 px-2 py-1 text-[10px] rounded bg-bg-tertiary text-text-secondary border border-border hover:text-white hover:border-[#666666] transition-colors">
          {t('techRadar.backToFull')}
        </button>
      )}
    </div>
  );
});

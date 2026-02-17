import { useState, useEffect, memo, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface RadarEntry {
  name: string;
  ring: 'adopt' | 'trial' | 'assess' | 'hold';
  quadrant: 'languages' | 'frameworks' | 'tools' | 'platforms';
  movement: 'up' | 'down' | 'stable' | 'new';
  signals: string[];
  decision_ref: number | null;
  score: number;
}

interface TechRadarData {
  generated_at: string;
  entries: RadarEntry[];
}

const CX = 300;
const CY = 300;

const RING_RADII: Record<string, number> = {
  adopt: 75,
  trial: 150,
  assess: 225,
  hold: 280,
};

const RING_LABELS = ['Adopt', 'Trial', 'Assess', 'Hold'];
const RING_KEYS = ['adopt', 'trial', 'assess', 'hold'];

const QUADRANT_CONFIG: Record<string, { label: string; startAngle: number; endAngle: number }> = {
  languages:  { label: 'Languages',  startAngle: Math.PI,       endAngle: Math.PI * 1.5 },
  frameworks: { label: 'Frameworks', startAngle: Math.PI * 1.5, endAngle: Math.PI * 2 },
  tools:      { label: 'Tools',      startAngle: Math.PI * 0.5, endAngle: Math.PI },
  platforms:  { label: 'Platforms',   startAngle: 0,             endAngle: Math.PI * 0.5 },
};

/** Deterministic hash for positioning dots within their sector. */
function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const ch = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + ch;
    hash |= 0;
  }
  return Math.abs(hash);
}

function getEntryPosition(entry: RadarEntry): { x: number; y: number } {
  const ring = RING_RADII[entry.ring];
  const prevRing = entry.ring === 'adopt' ? 0
    : entry.ring === 'trial' ? RING_RADII.adopt
    : entry.ring === 'assess' ? RING_RADII.trial
    : RING_RADII.assess;

  const quad = QUADRANT_CONFIG[entry.quadrant];
  const h = hashString(entry.name);

  // Radius: place between prev ring edge and current ring edge with some padding
  const radFraction = ((h % 1000) / 1000) * 0.7 + 0.15;
  const radius = prevRing + (ring - prevRing) * radFraction;

  // Angle: spread within the quadrant arc with padding
  const angleFraction = (((h >> 10) % 1000) / 1000) * 0.7 + 0.15;
  const angle = quad.startAngle + (quad.endAngle - quad.startAngle) * angleFraction;

  return {
    x: CX + radius * Math.cos(angle),
    y: CY + radius * Math.sin(angle),
  };
}

interface TooltipState {
  entry: RadarEntry;
  x: number;
  y: number;
}

const MovementIndicator = memo(function MovementIndicator({
  x,
  y,
  movement,
}: {
  x: number;
  y: number;
  movement: RadarEntry['movement'];
}) {
  const offsetY = -9;
  const cx = x;
  const cy = y + offsetY;

  if (movement === 'up') {
    return (
      <polygon
        points={`${cx},${cy - 4} ${cx - 3},${cy + 2} ${cx + 3},${cy + 2}`}
        fill="#22C55E"
      />
    );
  }
  if (movement === 'down') {
    return (
      <polygon
        points={`${cx},${cy + 4} ${cx - 3},${cy - 2} ${cx + 3},${cy - 2}`}
        fill="#EF4444"
      />
    );
  }
  if (movement === 'new') {
    return (
      <polygon
        points={`${cx},${cy - 4} ${cx + 3},${cy} ${cx},${cy + 4} ${cx - 3},${cy}`}
        fill="#D4AF37"
      />
    );
  }
  return null;
});

export const TechRadar = memo(function TechRadar() {
  const [data, setData] = useState<TechRadarData | null>(null);
  const [loading, setLoading] = useState(true);
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const result = await invoke<TechRadarData>('get_tech_radar');
        setData(result);
      } catch {
        // Tech radar is optional
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  const handleMouseEnter = useCallback((entry: RadarEntry, x: number, y: number) => {
    setTooltip({ entry, x, y });
  }, []);

  const handleMouseLeave = useCallback(() => {
    setTooltip(null);
  }, []);

  const positionedEntries = useMemo(() => {
    if (!data) return [];
    return data.entries.map((entry) => ({
      entry,
      pos: getEntryPosition(entry),
    }));
  }, [data]);

  if (loading) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <div className="text-xs text-gray-500">Loading tech radar...</div>
      </div>
    );
  }

  if (!data || data.entries.length === 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-8 text-center">
        <div className="text-sm text-gray-500">No radar data available</div>
        <div className="text-xs text-gray-600 mt-1">
          Radar populates as decisions and signals accumulate
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {/* Header */}
      <div className="px-5 py-4 border-b border-border flex items-center gap-3">
        <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
          <span className="text-sm text-gray-400">R</span>
        </div>
        <div>
          <h2 className="font-medium text-white text-sm">Tech Radar</h2>
          <p className="text-xs text-gray-500">
            {data.entries.length} technolog{data.entries.length !== 1 ? 'ies' : 'y'} tracked
          </p>
        </div>
      </div>

      {/* Radar SVG */}
      <div className="p-4 flex justify-center">
        <svg
          viewBox="0 0 600 600"
          className="w-full max-w-[560px]"
          style={{ fontFamily: 'Inter, sans-serif' }}
        >
          {/* Background */}
          <rect x="0" y="0" width="600" height="600" fill="#0A0A0A" rx="8" />

          {/* Crosshair lines */}
          <line x1={CX} y1={CY - 290} x2={CX} y2={CY + 290} stroke="#2A2A2A" strokeWidth="1" />
          <line x1={CX - 290} y1={CY} x2={CX + 290} y2={CY} stroke="#2A2A2A" strokeWidth="1" />

          {/* Concentric rings */}
          {RING_KEYS.map((ring) => (
            <circle
              key={ring}
              cx={CX}
              cy={CY}
              r={RING_RADII[ring]}
              fill="none"
              stroke="#2A2A2A"
              strokeWidth="1"
            />
          ))}

          {/* Ring labels along the right axis */}
          {RING_KEYS.map((ring, i) => {
            const prevR = i === 0 ? 0 : RING_RADII[RING_KEYS[i - 1]];
            const midR = (prevR + RING_RADII[ring]) / 2;
            return (
              <text
                key={ring}
                x={CX + midR}
                y={CY - 6}
                textAnchor="middle"
                fill="#666666"
                fontSize="9"
                fontWeight="500"
              >
                {RING_LABELS[i]}
              </text>
            );
          })}

          {/* Quadrant labels */}
          <text x={80} y={30} textAnchor="middle" fill="#A0A0A0" fontSize="11" fontWeight="600">
            Languages
          </text>
          <text x={520} y={30} textAnchor="middle" fill="#A0A0A0" fontSize="11" fontWeight="600">
            Frameworks
          </text>
          <text x={80} y={590} textAnchor="middle" fill="#A0A0A0" fontSize="11" fontWeight="600">
            Tools
          </text>
          <text x={520} y={590} textAnchor="middle" fill="#A0A0A0" fontSize="11" fontWeight="600">
            Platforms
          </text>

          {/* Entries */}
          {positionedEntries.map(({ entry, pos }) => (
            <g
              key={entry.name}
              onMouseEnter={() => handleMouseEnter(entry, pos.x, pos.y)}
              onMouseLeave={handleMouseLeave}
              style={{ cursor: 'pointer' }}
            >
              <circle cx={pos.x} cy={pos.y} r="5" fill="#FFFFFF" opacity="0.9" />
              <circle cx={pos.x} cy={pos.y} r="10" fill="transparent" />
              {entry.movement !== 'stable' && (
                <MovementIndicator x={pos.x} y={pos.y} movement={entry.movement} />
              )}
            </g>
          ))}

          {/* Tooltip */}
          {tooltip && (
            <g>
              {/* Position tooltip to stay within viewBox */}
              {(() => {
                const tx = tooltip.x > 400 ? tooltip.x - 170 : tooltip.x + 15;
                const ty = tooltip.y > 450 ? tooltip.y - 80 : tooltip.y + 15;
                const ringLabel =
                  tooltip.entry.ring.charAt(0).toUpperCase() + tooltip.entry.ring.slice(1);
                const quadLabel =
                  tooltip.entry.quadrant.charAt(0).toUpperCase() +
                  tooltip.entry.quadrant.slice(1);

                return (
                  <g>
                    <rect
                      x={tx}
                      y={ty}
                      width="160"
                      height={56 + Math.min(tooltip.entry.signals.length, 3) * 14}
                      rx="6"
                      fill="#1F1F1F"
                      stroke="#2A2A2A"
                      strokeWidth="1"
                    />
                    <text x={tx + 8} y={ty + 16} fill="#FFFFFF" fontSize="11" fontWeight="600">
                      {tooltip.entry.name}
                    </text>
                    <text x={tx + 8} y={ty + 30} fill="#A0A0A0" fontSize="9">
                      {ringLabel} / {quadLabel}
                    </text>
                    <text x={tx + 8} y={ty + 44} fill="#666666" fontSize="9">
                      Score: {tooltip.entry.score.toFixed(2)} | Movement:{' '}
                      {tooltip.entry.movement}
                    </text>
                    {tooltip.entry.signals.slice(0, 3).map((signal, i) => (
                      <text
                        key={i}
                        x={tx + 8}
                        y={ty + 58 + i * 14}
                        fill="#666666"
                        fontSize="8"
                      >
                        - {signal.length > 28 ? signal.slice(0, 28) + '...' : signal}
                      </text>
                    ))}
                  </g>
                );
              })()}
            </g>
          )}
        </svg>
      </div>

      {/* Legend */}
      <div className="px-5 py-3 border-t border-border flex items-center gap-5 text-[10px] text-gray-500">
        <div className="flex items-center gap-1.5">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <polygon points="5,1 2,7 8,7" fill="#22C55E" />
          </svg>
          <span>Moving in</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <polygon points="5,9 2,3 8,3" fill="#EF4444" />
          </svg>
          <span>Moving out</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <polygon points="5,1 9,5 5,9 1,5" fill="#D4AF37" />
          </svg>
          <span>New</span>
        </div>
        <div className="flex items-center gap-1.5">
          <svg width="8" height="8" viewBox="0 0 8 8">
            <circle cx="4" cy="4" r="3" fill="#FFFFFF" />
          </svg>
          <span>Stable</span>
        </div>
        {data.generated_at && (
          <span className="ml-auto text-gray-600">
            Generated {new Date(data.generated_at).toLocaleDateString()}
          </span>
        )}
      </div>
    </div>
  );
});

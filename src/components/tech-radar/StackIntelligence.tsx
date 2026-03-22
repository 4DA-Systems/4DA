import { useMemo, memo } from 'react';

import type { RadarEntry } from './RadarSVG';

export interface StackIntelligenceProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

// ---------------------------------------------------------------------------
// Health computation
// ---------------------------------------------------------------------------

interface HealthGrade {
  grade: string;
  color: string;
}

interface DimensionGrade {
  name: string;
  grade: string;
  color: string;
}

function computeHealthGrade(entries: RadarEntry[]): HealthGrade {
  const total = entries.length;
  if (!total) return { grade: 'N/A', color: '#8A8A8A' };

  const adoptPct = entries.filter(e => e.ring === 'adopt').length / total;
  const holdPct = entries.filter(e => e.ring === 'hold').length / total;
  const downPct = entries.filter(e => e.movement === 'down').length / total;
  const score = (adoptPct * 60) + ((1 - holdPct) * 30) + ((1 - downPct) * 10);

  if (score >= 90) return { grade: 'A', color: '#22C55E' };
  if (score >= 80) return { grade: 'A-', color: '#22C55E' };
  if (score >= 70) return { grade: 'B+', color: '#D4AF37' };
  if (score >= 60) return { grade: 'B', color: '#D4AF37' };
  if (score >= 50) return { grade: 'C', color: '#EF4444' };
  return { grade: 'D', color: '#EF4444' };
}

function computeDimensions(entries: RadarEntry[]): DimensionGrade[] {
  const total = entries.length;
  if (!total) return [];

  const holdPct = entries.filter(e => e.ring === 'hold').length / total;
  const securityGrade = holdPct <= 0.1 ? 'A' : holdPct <= 0.25 ? 'B' : 'C';

  const upPct = entries.filter(e => e.movement === 'up' || e.movement === 'new').length / total;
  const freshnessGrade = upPct >= 0.3 ? 'A' : upPct >= 0.15 ? 'B' : 'C';

  const adoptPct = entries.filter(e => e.ring === 'adopt' || e.ring === 'trial').length / total;
  const maintenanceGrade = adoptPct >= 0.6 ? 'A' : adoptPct >= 0.4 ? 'B' : 'C';

  const gradeColor = (g: string) => g === 'A' ? '#22C55E' : g === 'B' ? '#D4AF37' : '#EF4444';

  return [
    { name: 'Security', grade: securityGrade, color: gradeColor(securityGrade) },
    { name: 'Freshness', grade: freshnessGrade, color: gradeColor(freshnessGrade) },
    { name: 'Maintenance', grade: maintenanceGrade, color: gradeColor(maintenanceGrade) },
  ];
}

// ---------------------------------------------------------------------------
// Tier definitions
// ---------------------------------------------------------------------------

const TIER_DEFS = [
  { name: 'Core Stack', ring: 'adopt', color: '#22C55E', barColor: '#22C55E' },
  { name: 'Expanding', ring: 'trial', color: '#D4AF37', barColor: '#D4AF37' },
  { name: 'Watching', ring: 'assess', color: '#8A8A8A', barColor: '#555555' },
] as const;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const StackIntelligence = memo(function StackIntelligence({
  entries,
  onEntryClick,
}: StackIntelligenceProps) {
  const health = useMemo(() => computeHealthGrade(entries), [entries]);
  const dimensions = useMemo(() => computeDimensions(entries), [entries]);

  const attentionItems = useMemo(
    () => entries.filter(e => e.ring === 'hold' || e.movement === 'down'),
    [entries],
  );

  const tiers = useMemo(
    () =>
      TIER_DEFS.map(def => ({
        ...def,
        entries: entries
          .filter(e => e.ring === def.ring)
          .sort((a, b) => b.score - a.score),
      })),
    [entries],
  );

  const movingEntries = useMemo(
    () => entries.filter(e => e.movement !== 'stable'),
    [entries],
  );

  return (
    <div style={{ overflowY: 'auto', fontFamily: 'Inter, sans-serif', width: '100%', maxWidth: '560px' }}>
      {/* Section 1: Health Pulse */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '24px', padding: '20px 24px', borderBottom: '1px solid #2A2A2A' }}>
        <div style={{ fontSize: '48px', fontWeight: 700, color: health.color, lineHeight: 1 }}>{health.grade}</div>
        <div>
          <div style={{ fontSize: '11px', color: '#8A8A8A', textTransform: 'uppercase', letterSpacing: '1px', marginBottom: '8px' }}>
            Stack Health
          </div>
          <div style={{ display: 'flex', gap: '12px' }}>
            {dimensions.map(d => (
              <span key={d.name} style={{ fontSize: '12px', color: '#A0A0A0' }}>
                {d.name}: <span style={{ color: d.color, fontWeight: 600 }}>{d.grade}</span>
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* Section 2: Needs Attention */}
      {attentionItems.length > 0 && (
        <div style={{ padding: '16px 24px', borderBottom: '1px solid #2A2A2A' }}>
          <div style={{ fontSize: '11px', color: '#EF4444', textTransform: 'uppercase', letterSpacing: '1px', marginBottom: '12px' }}>
            Needs Attention ({attentionItems.length})
          </div>
          {attentionItems.map(entry => (
            <div
              key={entry.name}
              onClick={() => onEntryClick(entry)}
              role="button"
              tabIndex={0}
              onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onEntryClick(entry); } }}
              style={{ cursor: 'pointer', padding: '8px 12px', borderRadius: '6px', border: '1px solid #2A2A2A', marginBottom: '6px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
            >
              <span style={{ color: '#FFFFFF', fontSize: '13px' }}>{entry.name}</span>
              <span style={{ fontSize: '11px', color: '#EF4444' }}>
                {entry.movement === 'down' ? '\u2193 declining' : 'hold'}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Section 3: Stack Grid */}
      {tiers.map(tier => (
        tier.entries.length > 0 && (
          <div key={tier.name} style={{ padding: '16px 24px' }}>
            <div style={{ fontSize: '11px', color: tier.color, textTransform: 'uppercase', letterSpacing: '1px', marginBottom: '10px' }}>
              {tier.name} ({tier.entries.length})
            </div>
            {tier.entries.map(entry => (
              <div
                key={entry.name}
                onClick={() => onEntryClick(entry)}
                role="button"
                tabIndex={0}
                onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onEntryClick(entry); } }}
                style={{ display: 'flex', alignItems: 'center', gap: '12px', padding: '6px 12px', cursor: 'pointer', borderRadius: '4px' }}
                onMouseEnter={e => { (e.currentTarget as HTMLElement).style.backgroundColor = '#1F1F1F'; }}
                onMouseLeave={e => { (e.currentTarget as HTMLElement).style.backgroundColor = 'transparent'; }}
              >
                <div style={{ width: '120px', display: 'flex', gap: '1px', flexShrink: 0 }}>
                  {Array.from({ length: 10 }, (_, i) => (
                    <div
                      key={i}
                      style={{
                        width: '10px',
                        height: '14px',
                        borderRadius: '2px',
                        backgroundColor: i < Math.round(entry.score * 10) ? tier.barColor : '#1A1A1A',
                      }}
                    />
                  ))}
                </div>
                <span style={{ flex: 1, color: '#FFFFFF', fontSize: '13px', minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {entry.name}
                </span>
                <span style={{ fontSize: '11px', color: '#555555', width: '60px', textAlign: 'right', flexShrink: 0 }}>
                  {entry.quadrant}
                </span>
                <span style={{ fontSize: '12px', color: '#8A8A8A', fontFamily: 'monospace', width: '40px', textAlign: 'right', flexShrink: 0 }}>
                  {entry.score.toFixed(2)}
                </span>
                {entry.movement === 'up' && <span style={{ color: '#22C55E', fontSize: '11px', flexShrink: 0 }}>{'\u2191'}</span>}
                {entry.movement === 'down' && <span style={{ color: '#EF4444', fontSize: '11px', flexShrink: 0 }}>{'\u2193'}</span>}
                {entry.movement === 'new' && <span style={{ color: '#D4AF37', fontSize: '11px', flexShrink: 0 }}>{'\u2726'}</span>}
              </div>
            ))}
          </div>
        )
      ))}

      {/* Section 4: Movement */}
      {movingEntries.length > 0 && (
        <div style={{ padding: '16px 24px', borderTop: '1px solid #2A2A2A' }}>
          <div style={{ fontSize: '11px', color: '#8A8A8A', textTransform: 'uppercase', letterSpacing: '1px', marginBottom: '10px' }}>
            Movement
          </div>
          {movingEntries.map(entry => (
            <div key={entry.name} style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 12px', fontSize: '13px' }}>
              <span style={{ color: entry.movement === 'up' ? '#22C55E' : entry.movement === 'down' ? '#EF4444' : '#D4AF37' }}>
                {entry.movement === 'up' ? '\u2191' : entry.movement === 'down' ? '\u2193' : '\u2726'}
              </span>
              <span style={{ color: '#FFFFFF', width: '120px', flexShrink: 0 }}>{entry.name}</span>
              <span style={{ color: '#555555', fontSize: '11px' }}>
                {entry.movement === 'up' ? 'accelerating' : entry.movement === 'down' ? 'declining' : 'new'} {'\u00B7'} {entry.signals.length} signals
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
});

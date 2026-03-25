import { useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';

import type { RadarEntry } from './RadarSVG';

export interface StackIntelligenceProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

// ---------------------------------------------------------------------------
// Ring definitions
// ---------------------------------------------------------------------------

const RINGS = [
  { key: 'adopt', labelKey: 'stack.tier.adopt', color: '#22C55E', label: 'Core Stack' },
  { key: 'trial', labelKey: 'stack.tier.trial', color: '#D4AF37', label: 'Expanding' },
  { key: 'assess', labelKey: 'stack.tier.assess', color: '#8A8A8A', label: 'Watching' },
  { key: 'hold', labelKey: 'stack.tier.hold', color: '#EF4444', label: 'On Hold' },
] as const;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function movementSymbol(m: RadarEntry['movement']): { icon: string; color: string } {
  switch (m) {
    case 'up': return { icon: '\u2191', color: '#22C55E' };
    case 'down': return { icon: '\u2193', color: '#EF4444' };
    case 'new': return { icon: '\u2726', color: '#D4AF37' };
    case 'stable': return { icon: '', color: 'transparent' };
  }
}

function quadrantLabel(q: string): string {
  return q.charAt(0).toUpperCase() + q.slice(1);
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function RingDistribution({ entries }: { entries: RadarEntry[] }) {
  const { t } = useTranslation();
  const total = entries.length;

  return (
    <div className="space-y-1.5">
      {RINGS.map(ring => {
        const count = entries.filter(e => e.ring === ring.key).length;
        const pct = total > 0 ? (count / total) * 100 : 0;
        return (
          <div key={ring.key} className="flex items-center gap-2">
            <span className="text-[10px] w-16 flex-shrink-0" style={{ color: ring.color }}>
              {t(ring.labelKey, ring.label)}
            </span>
            <div className="flex-1 h-1.5 bg-[#1A1A1A] rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-500"
                style={{ width: `${pct}%`, backgroundColor: ring.color, opacity: 0.7 }}
              />
            </div>
            <span className="text-[10px] text-text-muted font-mono w-6 text-end">
              {count}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function EntryRow({ entry, onEntryClick, barColor }: {
  entry: RadarEntry;
  onEntryClick: (entry: RadarEntry) => void;
  barColor: string;
}) {
  const mv = movementSymbol(entry.movement);
  return (
    <div
      onClick={() => onEntryClick(entry)}
      role="button"
      tabIndex={0}
      onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onEntryClick(entry); } }}
      className="flex items-center gap-3 px-3 py-1.5 rounded cursor-pointer hover:bg-[#1F1F1F] transition-colors group"
    >
      {/* Score bar */}
      <div className="w-20 flex gap-px flex-shrink-0">
        {Array.from({ length: 10 }, (_, i) => (
          <div
            key={i}
            className="h-3 rounded-sm flex-1"
            style={{ backgroundColor: i < Math.round(entry.score * 10) ? barColor : '#1A1A1A' }}
          />
        ))}
      </div>
      {/* Name */}
      <span className="flex-1 text-[13px] text-white truncate min-w-0 group-hover:text-white">
        {entry.name}
      </span>
      {/* Quadrant */}
      <span className="text-[10px] text-text-muted w-16 text-end flex-shrink-0 hidden sm:block">
        {quadrantLabel(entry.quadrant)}
      </span>
      {/* Score */}
      <span className="text-[11px] text-text-muted font-mono w-8 text-end flex-shrink-0">
        {entry.score.toFixed(2)}
      </span>
      {/* Movement */}
      <span className="w-3 text-center flex-shrink-0 text-[11px]" style={{ color: mv.color }}>
        {mv.icon}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export const StackIntelligence = memo(function StackIntelligence({
  entries,
  onEntryClick,
}: StackIntelligenceProps) {
  const { t } = useTranslation();

  const attentionItems = useMemo(
    () => entries.filter(e => e.ring === 'hold' || e.movement === 'down'),
    [entries],
  );

  const tiers = useMemo(
    () => RINGS.slice(0, 3).map(ring => ({
      ...ring,
      entries: entries
        .filter(e => e.ring === ring.key)
        .sort((a, b) => b.score - a.score),
    })),
    [entries],
  );

  const movingEntries = useMemo(
    () => entries.filter(e => e.movement !== 'stable').sort((a, b) => {
      const order = { up: 0, new: 1, down: 2, stable: 3 };
      return order[a.movement] - order[b.movement];
    }),
    [entries],
  );

  const movingUp = entries.filter(e => e.movement === 'up').length;
  const movingDown = entries.filter(e => e.movement === 'down').length;
  const newEntries = entries.filter(e => e.movement === 'new').length;

  return (
    <div className="w-full max-w-[640px]">
      {/* Overview Stats */}
      <div className="grid grid-cols-3 gap-3 px-4 py-4 border-b border-border">
        <div className="text-center">
          <div className="text-xl font-semibold text-white">{entries.length}</div>
          <div className="text-[10px] text-text-muted uppercase tracking-wider">
            {t('stack.totalTracked', 'Technologies')}
          </div>
        </div>
        <div className="text-center">
          <div className="text-xl font-semibold text-white">
            {movingUp > 0 && <span className="text-green-400">{movingUp}</span>}
            {movingUp > 0 && movingDown > 0 && <span className="text-text-muted mx-0.5">/</span>}
            {movingDown > 0 && <span className="text-red-400">{movingDown}</span>}
            {movingUp === 0 && movingDown === 0 && <span className="text-text-muted">0</span>}
          </div>
          <div className="text-[10px] text-text-muted uppercase tracking-wider">
            {t('stack.movingLabel', 'Moving')}
          </div>
        </div>
        <div className="text-center">
          <div className="text-xl font-semibold text-accent-gold">{newEntries}</div>
          <div className="text-[10px] text-text-muted uppercase tracking-wider">
            {t('stack.newLabel', 'New')}
          </div>
        </div>
      </div>

      {/* Ring Distribution */}
      <div className="px-5 py-4 border-b border-border">
        <RingDistribution entries={entries} />
      </div>

      {/* Attention Section */}
      {attentionItems.length > 0 && (
        <div className="px-5 py-3 border-b border-border">
          <div className="text-[10px] text-red-400 uppercase tracking-wider font-medium mb-2">
            {t('stack.needsAttention', { count: attentionItems.length })}
          </div>
          {attentionItems.map(entry => (
            <div
              key={entry.name}
              onClick={() => onEntryClick(entry)}
              role="button"
              tabIndex={0}
              onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onEntryClick(entry); } }}
              className="flex items-center justify-between px-3 py-2 rounded cursor-pointer hover:bg-[#1F1F1F] transition-colors mb-1"
            >
              <span className="text-[13px] text-white">{entry.name}</span>
              <span className="text-[11px] text-red-400">
                {entry.movement === 'down' ? `\u2193 ${t('stack.declining')}` : t('stack.hold')}
              </span>
            </div>
          ))}
        </div>
      )}

      {/* Stack Tiers */}
      {tiers.map(tier => (
        tier.entries.length > 0 && (
          <div key={tier.key} className="px-4 py-3 border-b border-border last:border-b-0">
            <div className="flex items-center gap-2 mb-2 px-1">
              <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: tier.color }} />
              <span className="text-[10px] uppercase tracking-wider font-medium" style={{ color: tier.color }}>
                {t(tier.labelKey, tier.label)}
              </span>
              <span className="text-[10px] text-text-muted">({tier.entries.length})</span>
            </div>
            {tier.entries.map(entry => (
              <EntryRow key={entry.name} entry={entry} onEntryClick={onEntryClick} barColor={tier.color} />
            ))}
          </div>
        )
      ))}

      {/* Movement Summary */}
      {movingEntries.length > 0 && (
        <div className="px-5 py-3 border-t border-border">
          <div className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-2">
            {t('stack.movement', 'Movement')}
          </div>
          {movingEntries.map(entry => {
            const mv = movementSymbol(entry.movement);
            return (
              <div key={entry.name} className="flex items-center gap-2 px-3 py-1 text-[12px]">
                <span style={{ color: mv.color }}>{mv.icon}</span>
                <span className="text-white w-24 flex-shrink-0 truncate">{entry.name}</span>
                <span className="text-text-muted text-[11px]">
                  {entry.movement === 'up' ? t('stack.accelerating') : entry.movement === 'down' ? t('stack.declining') : t('stack.new')}
                </span>
                {entry.signals.length > 0 && (
                  <span className="text-text-muted text-[10px] ms-auto">
                    {t('stack.signalCount', { count: entry.signals.length })}
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});

import { useMemo, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { filterNoteworthy, sortByMomentum, generateNarrative, isInUserStack } from './momentum-utils';

// ---------------------------------------------------------------------------
// Movement badge
// ---------------------------------------------------------------------------

const MOVEMENT_STYLE: Record<string, { label: string; border: string; text: string; bg: string }> = {
  up:     { label: 'Rising',    border: 'border-green-500/30', text: 'text-green-400', bg: 'bg-green-500/10' },
  new:    { label: 'New',       border: 'border-amber-500/30', text: 'text-accent-gold', bg: 'bg-amber-500/10' },
  down:   { label: 'Declining', border: 'border-red-500/30',   text: 'text-red-400',   bg: 'bg-red-500/10' },
  stable: { label: 'Core',      border: 'border-border',       text: 'text-text-muted', bg: 'bg-bg-tertiary' },
};

// ---------------------------------------------------------------------------
// Narrative Card
// ---------------------------------------------------------------------------

const NarrativeCard = memo(function NarrativeCard({
  entry,
  narrative,
  isStack,
  onClick,
  index,
}: {
  entry: RadarEntry;
  narrative: string;
  isStack: boolean;
  onClick: () => void;
  index: number;
}) {
  const { t } = useTranslation();
  const style = MOVEMENT_STYLE[entry.movement] ?? MOVEMENT_STYLE.stable!;

  return (
    <button
      onClick={onClick}
      className={`w-full text-start rounded-lg border ${style.border} bg-bg-secondary p-4
        hover:bg-[#1A1A1A] transition-all group focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent-gold`}
      style={{ animation: `slideInRight 0.4s ease-out ${index * 60}ms both` }}
    >
      <div className="flex items-center gap-2 mb-1.5">
        <h4 className="text-sm font-semibold text-white group-hover:text-accent-gold transition-colors">
          {entry.name}
        </h4>
        {isStack && (
          <span className="w-1.5 h-1.5 rounded-full bg-accent-gold flex-shrink-0" title={t('momentum.inYourStack')} />
        )}
        <span className={`ms-auto text-[10px] px-1.5 py-0.5 rounded font-medium ${style.text} ${style.bg} flex-shrink-0`}>
          {t(`momentum.movement.${entry.movement}`, style.label)}
        </span>
      </div>
      <p className="text-xs text-text-secondary leading-relaxed">{narrative}</p>
      {entry.signals.length > 1 && (
        <span className="text-[10px] text-text-muted mt-2 inline-block">
          {t('momentum.signalCount', { count: entry.signals.length })}
        </span>
      )}
    </button>
  );
});

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export interface MomentumNarrativesProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

export const MomentumNarratives = memo(function MomentumNarratives({
  entries,
  userStack,
  onEntryClick,
}: MomentumNarrativesProps) {
  const { t } = useTranslation();

  const cards = useMemo(() => {
    const noteworthy = filterNoteworthy(entries, userStack);
    return sortByMomentum(noteworthy).slice(0, 8);
  }, [entries, userStack]);

  const handleClick = useCallback(
    (entry: RadarEntry) => () => onEntryClick(entry),
    [onEntryClick],
  );

  if (cards.length === 0) {
    return (
      <div className="px-5 py-6 text-center border-b border-border">
        <p className="text-sm text-text-muted">{t('momentum.noMovement')}</p>
      </div>
    );
  }

  return (
    <div className="px-4 py-4 border-b border-border">
      <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3 px-1">
        {t('momentum.whatsHappening')}
      </h3>
      <div className="grid grid-cols-1 gap-2">
        {cards.map((entry, i) => (
          <NarrativeCard
            key={entry.name}
            entry={entry}
            narrative={generateNarrative(entry, isInUserStack(entry.name, userStack), t)}
            isStack={isInUserStack(entry.name, userStack)}
            onClick={handleClick(entry)}
            index={i}
          />
        ))}
      </div>
    </div>
  );
});

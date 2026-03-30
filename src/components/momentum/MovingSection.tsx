// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useMemo, useCallback, useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { filterNoteworthy, sortByMomentum, generateNarrative, isInUserStack } from './momentum-utils';

// ---------------------------------------------------------------------------
// Movement badge styles
// ---------------------------------------------------------------------------

const MOVEMENT_STYLE: Record<string, { border: string; text: string; bg: string }> = {
  up:     { border: 'border-green-500/30', text: 'text-green-400', bg: 'bg-green-500/10' },
  new:    { border: 'border-amber-500/30', text: 'text-accent-gold', bg: 'bg-amber-500/10' },
  down:   { border: 'border-red-500/30',   text: 'text-red-400',   bg: 'bg-red-500/10' },
  stable: { border: 'border-border',       text: 'text-text-muted', bg: 'bg-bg-tertiary' },
};

// ---------------------------------------------------------------------------
// Feedback buttons (B3 — autophagy loop)
// ---------------------------------------------------------------------------

function FeedbackButtons({ topic }: { topic: string }) {
  const [given, setGiven] = useState<'up' | 'down' | null>(null);

  const handleFeedback = useCallback((positive: boolean) => {
    // Record via the existing interaction pipeline — uses topic as the item context
    void cmd('ace_record_interaction', {
      item_id: 0,
      action_type: positive ? 'save' : 'dismiss',
      action_data: null,
      item_topics: [topic],
      item_source: 'momentum',
    }).catch(() => {});
    setGiven(positive ? 'up' : 'down');
  }, [topic]);

  if (given !== null) {
    return <span className="text-[10px] text-text-muted">{given === 'up' ? '\u2713' : '\u2717'}</span>;
  }

  return (
    <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
      <button
        onClick={e => { e.stopPropagation(); handleFeedback(true); }}
        className="text-text-muted hover:text-green-400 transition-colors text-xs px-1"
        title="Useful"
      >
        {'\u25B2'}
      </button>
      <button
        onClick={e => { e.stopPropagation(); handleFeedback(false); }}
        className="text-text-muted hover:text-red-400 transition-colors text-xs px-1"
        title="Not useful"
      >
        {'\u25BC'}
      </button>
    </div>
  );
}

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
        <h4 className="text-sm font-semibold text-white group-hover:text-accent-gold transition-colors truncate">
          {entry.name}
        </h4>
        {isStack && (
          <span className="w-1.5 h-1.5 rounded-full bg-accent-gold flex-shrink-0" title={t('momentum.inYourStack')} />
        )}
        <FeedbackButtons topic={entry.name} />
        <span className={`ms-auto text-[10px] px-1.5 py-0.5 rounded font-medium ${style.text} ${style.bg} flex-shrink-0`}>
          {t(`momentum.movement.${entry.movement}`, { defaultValue: entry.movement })}
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

export interface MovingSectionProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

export const MovingSection = memo(function MovingSection({
  entries,
  userStack,
  onEntryClick,
}: MovingSectionProps) {
  const { t } = useTranslation();

  // LLM narratives (B2 — loaded async, fallback to signal-based)
  const [llmNarratives, setLlmNarratives] = useState<Record<string, string>>({});
  useEffect(() => {
    void cmd('generate_tech_narratives').then(result => {
      setLlmNarratives((result as { narratives: Record<string, string> }).narratives);
    }).catch(() => {}); // Graceful — signal-based fallback
  }, []);

  const cards = useMemo(() => {
    const noteworthy = filterNoteworthy(entries, userStack);
    return sortByMomentum(noteworthy).slice(0, 6);
  }, [entries, userStack]);

  const handleClick = useCallback(
    (entry: RadarEntry) => () => onEntryClick(entry),
    [onEntryClick],
  );

  const getNarrative = useCallback((entry: RadarEntry) => {
    // Use LLM narrative if available, otherwise fall back to signal-based
    const llmText = llmNarratives[entry.name.toLowerCase()];
    if (llmText !== undefined && llmText !== '') return llmText;
    return generateNarrative(entry, isInUserStack(entry.name, userStack), t);
  }, [llmNarratives, userStack, t]);

  if (cards.length === 0) return null;

  return (
    <section aria-label={t('momentum.whatsMoving')}>
      <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3 px-1">
        {t('momentum.whatsMoving')}
      </h3>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        {cards.map((entry, i) => (
          <NarrativeCard
            key={entry.name}
            entry={entry}
            narrative={getNarrative(entry)}
            isStack={isInUserStack(entry.name, userStack)}
            onClick={handleClick(entry)}
            index={i}
          />
        ))}
      </div>
    </section>
  );
});

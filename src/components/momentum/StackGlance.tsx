// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useMemo, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { RadarEntry } from '../tech-radar/RadarSVG';
import { isInUserStack, getStackHealth } from './momentum-utils';
import type { StackHealth } from './momentum-utils';

// ---------------------------------------------------------------------------
// Health dot colors
// ---------------------------------------------------------------------------

const HEALTH_DOT: Record<StackHealth, { color: string; title: string }> = {
  healthy:     { color: 'bg-green-400',  title: 'momentum.healthy' },
  noteworthy:  { color: 'bg-amber-400',  title: 'momentum.noteworthy' },
  attention:   { color: 'bg-red-400',    title: 'momentum.needsAttention' },
};

// ---------------------------------------------------------------------------
// Single tech chip
// ---------------------------------------------------------------------------

const StackChip = memo(function StackChip({
  entry,
  health,
  onClick,
}: {
  entry: RadarEntry;
  health: StackHealth;
  onClick: () => void;
}) {
  const { t } = useTranslation();
  const dot = HEALTH_DOT[health];
  const borderClass = health === 'attention' ? 'border-red-500/30'
    : health === 'noteworthy' ? 'border-amber-500/30'
    : 'border-border';

  return (
    <button
      onClick={onClick}
      className={`inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border ${borderClass}
        bg-bg-secondary hover:bg-[#1A1A1A] transition-colors text-sm text-white
        focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent-gold`}
      title={t(dot.title)}
    >
      <span className={`w-2 h-2 rounded-full ${dot.color} flex-shrink-0`} />
      <span className="truncate">{entry.name}</span>
    </button>
  );
});

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export interface StackGlanceProps {
  entries: RadarEntry[];
  userStack: string[];
  onEntryClick: (entry: RadarEntry) => void;
}

export const StackGlance = memo(function StackGlance({
  entries,
  userStack,
  onEntryClick,
}: StackGlanceProps) {
  const { t } = useTranslation();

  const stackEntries = useMemo(() => {
    // Get entries that match the user's stack, sorted: attention first, then noteworthy, then healthy
    const healthOrder: Record<StackHealth, number> = { attention: 0, noteworthy: 1, healthy: 2 };
    return entries
      .filter(e => isInUserStack(e.name, userStack))
      .map(e => ({ entry: e, health: getStackHealth(e) }))
      .sort((a, b) => healthOrder[a.health] - healthOrder[b.health]);
  }, [entries, userStack]);

  const handleClick = useCallback(
    (entry: RadarEntry) => () => onEntryClick(entry),
    [onEntryClick],
  );

  if (stackEntries.length === 0) return null;

  return (
    <section aria-label={t('momentum.stackGlance')}>
      <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3 px-1">
        {t('momentum.stackGlance')}
      </h3>
      <div className="flex flex-wrap gap-2">
        {stackEntries.map(({ entry, health }) => (
          <StackChip
            key={entry.name}
            entry={entry}
            health={health}
            onClick={handleClick(entry)}
          />
        ))}
      </div>
    </section>
  );
});

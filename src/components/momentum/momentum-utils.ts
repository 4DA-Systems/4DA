import type { RadarEntry } from '../tech-radar/RadarSVG';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function isInUserStack(name: string, stack: string[]): boolean {
  const lower = name.toLowerCase();
  return stack.some(s => s.toLowerCase() === lower);
}

/** Deterministic hash for consistent positioning. */
export function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash) + str.charCodeAt(i);
    hash |= 0;
  }
  return Math.abs(hash);
}

// ---------------------------------------------------------------------------
// Narrative Generation
// ---------------------------------------------------------------------------

export function generateNarrative(
  entry: RadarEntry,
  isStack: boolean,
  t: (key: string, opts?: Record<string, unknown>) => string,
): string {
  const mentionMatch = entry.signals.find(s => /\d+\s*mention/i.test(s));
  const mentionCount = mentionMatch?.match(/(\d+)/)?.[1] ?? null;
  const securitySignals = entry.signals.filter(s => /security|advisory|vuln|cve/i.test(s));
  const decisionSignals = entry.signals.filter(s => /decision|favor|rejected/i.test(s));

  const parts: string[] = [];

  switch (entry.movement) {
    case 'up':
      if (mentionCount !== null) parts.push(t('momentum.mentionsRecent', { count: mentionCount }));
      if (securitySignals.length > 0) parts.push(t('momentum.securityCount', { count: securitySignals.length }));
      if (parts.length === 0 && entry.signals.length > 0) parts.push(entry.signals[0]!);
      return parts.length > 0 ? parts.join(', ') : t('momentum.risingActivity');

    case 'new':
      if (isStack) return t('momentum.newInStack');
      return t('momentum.newAppearing', { count: entry.signals.length });

    case 'down':
      if (decisionSignals.length > 0) return decisionSignals[0]!;
      return t('momentum.decliningActivity');

    default:
      if (isStack && entry.signals.length > 0) return entry.signals[0]!;
      if (isStack) return t('momentum.coreStable');
      return t('momentum.stable');
  }
}

// ---------------------------------------------------------------------------
// Filtering & Sorting
// ---------------------------------------------------------------------------

/** Filter to entries worth showing as narrative cards. */
export function filterNoteworthy(entries: RadarEntry[], userStack: string[]): RadarEntry[] {
  return entries.filter(e => {
    if (e.movement !== 'stable') return true;
    if (isInUserStack(e.name, userStack) && e.signals.length > 0) return true;
    return false;
  });
}

/** Sort by momentum priority: new > up > down > stable-with-signals. */
export function sortByMomentum(entries: RadarEntry[]): RadarEntry[] {
  const order: Record<string, number> = { new: 0, up: 1, down: 2, stable: 3 };
  return [...entries].sort((a, b) => {
    const oDiff = (order[a.movement] ?? 3) - (order[b.movement] ?? 3);
    if (oDiff !== 0) return oDiff;
    return b.score - a.score;
  });
}

// ---------------------------------------------------------------------------
// Decision Prompt Derivation
// ---------------------------------------------------------------------------

export interface DerivedPrompt {
  id: string;
  text: string;
  subtext: string;
  type: 'track' | 'security' | 'declining' | 'version';
  entryName: string;
}

export function deriveDecisionPrompts(entries: RadarEntry[], userStack: string[]): DerivedPrompt[] {
  const prompts: DerivedPrompt[] = [];

  for (const e of entries) {
    // New tech not in stack — offer to track
    if (e.movement === 'new' && !isInUserStack(e.name, userStack)) {
      prompts.push({
        id: `track-${e.name}`,
        text: `Track ${e.name} as an interest?`,
        subtext: `Appeared in ${e.signals.length} signal${e.signals.length !== 1 ? 's' : ''} recently`,
        type: 'track',
        entryName: e.name,
      });
    }

    // Security signals on stack tech
    const secSignals = e.signals.filter(s => /security|advisory|vuln|cve/i.test(s));
    if (secSignals.length > 0 && isInUserStack(e.name, userStack)) {
      prompts.push({
        id: `security-${e.name}`,
        text: `${secSignals.length} security advisor${secSignals.length !== 1 ? 'ies' : 'y'} for ${e.name}`,
        subtext: secSignals[0]!,
        type: 'security',
        entryName: e.name,
      });
    }

    // Declining stack tech
    if (e.movement === 'down' && isInUserStack(e.name, userStack)) {
      prompts.push({
        id: `declining-${e.name}`,
        text: `${e.name} engagement is declining`,
        subtext: 'Still core to your work?',
        type: 'declining',
        entryName: e.name,
      });
    }
  }

  return prompts.slice(0, 5);
}

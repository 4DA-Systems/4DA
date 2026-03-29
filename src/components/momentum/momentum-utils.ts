import type { RadarEntry } from '../tech-radar/RadarSVG';
import type { DecisionWindow } from '../../types/autophagy';
import type { KnowledgeGap } from '../../types/innovation';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function isInUserStack(name: string, stack: string[]): boolean {
  const lower = name.toLowerCase();
  return stack.some(s => s.toLowerCase() === lower);
}

// ---------------------------------------------------------------------------
// Attention Item — unified type for the "What Needs Attention" section
// ---------------------------------------------------------------------------

export type AttentionKind = 'security' | 'decision_window' | 'knowledge_gap';

export interface AttentionItem {
  id: string;
  kind: AttentionKind;
  title: string;
  detail: string;
  urgency: number; // 0-1 for sorting
  entryName?: string; // links to a RadarEntry for drill-down
  windowId?: number;
}

export function buildAttentionItems(
  entries: RadarEntry[],
  userStack: string[],
  windows: DecisionWindow[],
  gaps: KnowledgeGap[],
  t: (key: string, opts?: Record<string, unknown>) => string,
): AttentionItem[] {
  const items: AttentionItem[] = [];

  // 1. Security advisories on YOUR stack techs
  for (const e of entries) {
    const secSignals = e.signals.filter(s => /security|advisory|vuln|cve/i.test(s));
    if (secSignals.length > 0 && isInUserStack(e.name, userStack)) {
      items.push({
        id: `sec-${e.name}`,
        kind: 'security',
        title: t('momentum.securityAlert', { name: e.name }),
        detail: secSignals[0]!,
        urgency: 0.95,
        entryName: e.name,
      });
    }
  }

  // 2. Decision windows that are open (sorted by urgency)
  const openWindows = windows.filter(w => w.status === 'open');
  for (const w of openWindows) {
    const timeLeft = getTimeRemaining(w.expires_at);
    items.push({
      id: `win-${w.id}`,
      kind: 'decision_window',
      title: w.title,
      detail: timeLeft
        ? t('momentum.windowExpiringDetail', { time: timeLeft, urgency: Math.round(w.urgency * 100) })
        : w.description ?? `${w.window_type} — urgency ${Math.round(w.urgency * 100)}%`,
      urgency: w.urgency,
      windowId: w.id,
    });
  }

  // 3. Knowledge gaps (critical and high only)
  const severeGaps = gaps.filter(g => g.gap_severity === 'critical' || g.gap_severity === 'high');
  for (const g of severeGaps) {
    items.push({
      id: `gap-${g.dependency}`,
      kind: 'knowledge_gap',
      title: t('momentum.knowledgeGap', { dependency: g.dependency }),
      detail: t('momentum.knowledgeGapDetail', { days: g.days_since_last_engagement }),
      urgency: g.gap_severity === 'critical' ? 0.85 : 0.6,
    });
  }

  // Sort by urgency descending, cap at 5
  items.sort((a, b) => b.urgency - a.urgency);
  return items.slice(0, 5);
}

// ---------------------------------------------------------------------------
// Time helpers
// ---------------------------------------------------------------------------

export function getTimeRemaining(expiresAt: string | null): string | null {
  if (!expiresAt) return null;
  const now = Date.now();
  const exp = new Date(expiresAt).getTime();
  const diff = exp - now;
  if (diff <= 0) return 'Expired';
  const hours = Math.floor(diff / (1000 * 60 * 60));
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d ${hours % 24}h left`;
  if (hours > 0) return `${hours}h left`;
  return `${Math.floor(diff / (1000 * 60))}m left`;
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
  const affinityMatch = entry.signals.find(s => /affinity\s*[\d.]+|engagement.*[\d.]+/i.test(s));
  const decisionMatch = entry.signals.find(s => /decision|Active decision/i.test(s));
  const securitySignals = entry.signals.filter(s => /security|advisory|vuln|cve/i.test(s));

  // Build meaningful narrative sentences from signal data
  const parts: string[] = [];

  switch (entry.movement) {
    case 'up': {
      if (mentionCount !== null) {
        parts.push(t('momentum.mentionsRecent', { count: mentionCount }));
      }
      if (affinityMatch) {
        parts.push(affinityMatch);
      }
      if (securitySignals.length > 0) {
        parts.push(t('momentum.securityCount', { count: securitySignals.length }));
      }
      if (parts.length === 0 && entry.signals.length > 0) {
        parts.push(entry.signals[0]!);
      }
      return parts.length > 0 ? parts.join('. ') : t('momentum.risingActivity');
    }

    case 'new': {
      if (isStack) return t('momentum.newInStack');
      if (decisionMatch) return decisionMatch;
      return t('momentum.newAppearing', { count: entry.signals.length });
    }

    case 'down': {
      if (securitySignals.length > 0) return securitySignals[0]!;
      const decisionSignals = entry.signals.filter(s => /decision|favor|rejected/i.test(s));
      if (decisionSignals.length > 0) return decisionSignals[0]!;
      return t('momentum.decliningActivity');
    }

    default: {
      if (isStack && entry.signals.length > 0) return entry.signals[0]!;
      if (isStack) return t('momentum.coreStable');
      return t('momentum.stable');
    }
  }
}

// ---------------------------------------------------------------------------
// Filtering & Sorting
// ---------------------------------------------------------------------------

/** Filter to entries worth showing as narrative cards (non-stable, or stable stack with signals). */
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
// Stack Health — determines dot color in the "Your Stack" section
// ---------------------------------------------------------------------------

export type StackHealth = 'healthy' | 'noteworthy' | 'attention';

export function getStackHealth(entry: RadarEntry): StackHealth {
  const hasSecuritySignal = entry.signals.some(s => /security|advisory|vuln|cve|deprecated/i.test(s));
  if (hasSecuritySignal) return 'attention';
  if (entry.movement === 'down') return 'noteworthy';
  return 'healthy';
}

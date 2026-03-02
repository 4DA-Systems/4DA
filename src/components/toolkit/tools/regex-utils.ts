export interface MatchResult {
  text: string;
  index: number;
  length: number;
  groups: Record<string, string | undefined>;
  groupEntries: string[];
}

export type Flag = 'g' | 'i' | 'm' | 's' | 'u';

export const FLAGS: { flag: Flag; label: string; title: string }[] = [
  { flag: 'g', label: 'g', title: 'Global' },
  { flag: 'i', label: 'i', title: 'Case insensitive' },
  { flag: 'm', label: 'm', title: 'Multiline' },
  { flag: 's', label: 's', title: 'Dotall' },
  { flag: 'u', label: 'u', title: 'Unicode' },
];

export const HIGHLIGHT_COLORS = [
  'rgba(234, 179, 8, 0.3)',
  'rgba(59, 130, 246, 0.3)',
  'rgba(34, 197, 94, 0.3)',
];

export function buildHighlightedSegments(
  text: string,
  matches: MatchResult[],
): { text: string; highlight: boolean; colorIndex: number }[] {
  if (matches.length === 0) return [{ text, highlight: false, colorIndex: 0 }];

  const segments: { text: string; highlight: boolean; colorIndex: number }[] = [];
  let cursor = 0;

  for (let i = 0; i < matches.length; i++) {
    const m = matches[i];
    if (m.index > cursor) {
      segments.push({ text: text.slice(cursor, m.index), highlight: false, colorIndex: 0 });
    }
    if (m.length > 0) {
      segments.push({ text: m.text, highlight: true, colorIndex: i % HIGHLIGHT_COLORS.length });
    }
    cursor = m.index + m.length;
  }

  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), highlight: false, colorIndex: 0 });
  }

  return segments;
}

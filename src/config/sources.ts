/** Canonical source registry — single source of truth for all source metadata. */

export type SourceId =
  | 'hackernews'
  | 'arxiv'
  | 'reddit'
  | 'github'
  | 'rss'
  | 'youtube'
  | 'twitter'
  | 'producthunt'
  | 'lobsters'
  | 'devto';

interface SourceMeta {
  /** Short display label (e.g. "HN") */
  label: string;
  /** Full display name (e.g. "Hacker News") */
  fullName: string;
  /** Tailwind color classes for badges */
  colorClass: string;
}

const SOURCES: Record<SourceId, SourceMeta> = {
  hackernews: { label: 'HN', fullName: 'Hacker News', colorClass: 'bg-orange-500/20 text-orange-300' },
  arxiv: { label: 'arXiv', fullName: 'arXiv', colorClass: 'bg-purple-500/20 text-purple-300' },
  reddit: { label: 'Reddit', fullName: 'Reddit', colorClass: 'bg-blue-500/20 text-blue-300' },
  github: { label: 'GitHub', fullName: 'GitHub', colorClass: 'bg-green-500/20 text-green-300' },
  rss: { label: 'RSS', fullName: 'RSS', colorClass: 'bg-yellow-500/20 text-yellow-300' },
  youtube: { label: 'YouTube', fullName: 'YouTube', colorClass: 'bg-red-500/20 text-red-300' },
  twitter: { label: 'Twitter', fullName: 'Twitter/X', colorClass: 'bg-sky-500/20 text-sky-300' },
  producthunt: { label: 'PH', fullName: 'Product Hunt', colorClass: 'bg-orange-500/20 text-orange-300' },
  lobsters: { label: 'Lobsters', fullName: 'Lobsters', colorClass: 'bg-pink-500/20 text-pink-300' },
  devto: { label: 'Dev.to', fullName: 'Dev.to', colorClass: 'bg-indigo-500/20 text-indigo-300' },
};

/** All valid source IDs */
export const ALL_SOURCE_IDS = new Set<SourceId>(Object.keys(SOURCES) as SourceId[]);

/** Get short label for a source (e.g. "HN"). Falls back to raw id. */
export function getSourceLabel(id: string): string {
  return (SOURCES as Record<string, SourceMeta>)[id]?.label ?? id;
}

/** Get full name for a source (e.g. "Hacker News"). Falls back to raw id. */
export function getSourceFullName(id: string): string {
  return (SOURCES as Record<string, SourceMeta>)[id]?.fullName ?? id;
}

/** Get Tailwind color classes for a source badge. Falls back to gray. */
export function getSourceColorClass(id: string): string {
  return (SOURCES as Record<string, SourceMeta>)[id]?.colorClass ?? 'bg-gray-500/20 text-gray-300';
}

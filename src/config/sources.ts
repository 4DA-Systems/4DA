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
  | 'devto'
  | 'cve'
  | 'osv';

interface SourceMeta {
  /** Short display label (e.g. "HN") */
  label: string;
  /** Full display name (e.g. "Hacker News") */
  fullName: string;
  /** Tailwind color classes for badges */
  colorClass: string;
}

const SOURCES: Record<SourceId, SourceMeta> = {
  hackernews: { label: 'HN', fullName: 'Hacker News', colorClass: 'bg-orange-500/20 text-orange-400' },
  arxiv: { label: 'arXiv', fullName: 'arXiv', colorClass: 'bg-purple-500/20 text-purple-400' },
  reddit: { label: 'Reddit', fullName: 'Reddit', colorClass: 'bg-blue-500/20 text-blue-400' },
  github: { label: 'GitHub', fullName: 'GitHub', colorClass: 'bg-gray-300/20 text-gray-300' },
  rss: { label: 'RSS', fullName: 'RSS', colorClass: 'bg-amber-500/20 text-amber-400' },
  youtube: { label: 'YouTube', fullName: 'YouTube', colorClass: 'bg-red-500/20 text-red-400' },
  twitter: { label: 'Twitter', fullName: 'Twitter/X', colorClass: 'bg-sky-500/20 text-sky-400' },
  producthunt: { label: 'PH', fullName: 'Product Hunt', colorClass: 'bg-orange-600/20 text-orange-300' },
  lobsters: { label: 'Lobsters', fullName: 'Lobsters', colorClass: 'bg-red-600/20 text-red-400' },
  devto: { label: 'Dev.to', fullName: 'Dev.to', colorClass: 'bg-green-500/20 text-green-400' },
  cve: { label: 'CVE', fullName: 'Security Advisories', colorClass: 'bg-red-600/20 text-red-400' },
  osv: { label: 'OSV', fullName: 'OSV Vulnerabilities', colorClass: 'bg-red-500/20 text-red-300' },
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
  return (SOURCES as Record<string, SourceMeta>)[id]?.colorClass ?? 'bg-gray-500/20 text-gray-400';
}

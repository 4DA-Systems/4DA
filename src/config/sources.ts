/** Source metadata — loaded from backend at startup, cached locally. */

import { cmd } from '../lib/commands';

interface SourceMeta {
  label: string;
  fullName: string;
  colorClass: string;
  category: string;
}

const DEFAULT_COLOR = 'bg-gray-500/20 text-gray-400';

const COLOR_MAP: Record<string, string> = {
  orange: 'bg-orange-500/20 text-orange-400',
  purple: 'bg-purple-500/20 text-purple-400',
  blue: 'bg-blue-500/20 text-blue-400',
  red: 'bg-red-500/20 text-red-400',
  green: 'bg-green-500/20 text-green-400',
  gray: 'bg-gray-500/20 text-gray-400',
  cyan: 'bg-cyan-500/20 text-cyan-300',
  yellow: 'bg-yellow-500/20 text-yellow-400',
  indigo: 'bg-indigo-500/20 text-indigo-300',
  amber: 'bg-amber-500/20 text-amber-400',
  sky: 'bg-sky-500/20 text-sky-400',
};

// Cache populated from backend
let sourcesCache = new Map<string, SourceMeta>();
let allIds = new Set<string>();

/** Load source metadata from the Rust backend. Call once at startup. */
export async function loadSourceMeta(): Promise<void> {
  try {
    const sources: Array<{
      type: string;
      name: string;
      category: string;
      label: string;
      color_hint: string;
    }> = await cmd('get_sources');
    sourcesCache.clear();
    allIds.clear();
    for (const s of sources) {
      sourcesCache.set(s.type, {
        label: s.label,
        fullName: s.name,
        colorClass: COLOR_MAP[s.color_hint] ?? DEFAULT_COLOR,
        category: s.category,
      });
      allIds.add(s.type);
    }
  } catch {
    // Backend not ready yet — use empty cache, will retry
  }
}

// Keep backward-compatible exports
export const ALL_SOURCE_IDS = allIds;
export function getSourceLabel(id: string): string {
  return sourcesCache.get(id)?.label ?? id;
}
export function getSourceFullName(id: string): string {
  return sourcesCache.get(id)?.fullName ?? id;
}
export function getSourceColorClass(id: string): string {
  return sourcesCache.get(id)?.colorClass ?? DEFAULT_COLOR;
}
export function getSourceCategory(id: string): string {
  return sourcesCache.get(id)?.category ?? 'general';
}
export function getSourcesByCategory(): Map<string, string[]> {
  const groups = new Map<string, string[]>();
  for (const [id, meta] of sourcesCache) {
    const cat = meta.category;
    const list = groups.get(cat) ?? [];
    list.push(id);
    groups.set(cat, list);
  }
  return groups;
}
export function isSourcesLoaded(): boolean {
  return sourcesCache.size > 0;
}

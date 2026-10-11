// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/** Source metadata — loaded from backend at startup, cached locally. */

import { cmd } from '../lib/commands';

interface SourceMeta {
  label: string;
  fullName: string;
  colorClass: string;
  category: string;
  /** The user's choice in `sources.enabled` (AD-054: interests are opt-in). */
  enabled: boolean;
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

// Cache populated from backend. Labels and colors cover every registered
// adapter (an item already on screen still needs its source's label), but
// only the sources the user reads are offered as filters.
const sourcesCache = new Map<string, SourceMeta>();
const enabledIds = new Set<string>();

function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/** Load source metadata from the Rust backend. Call once at startup. */
export async function loadSourceMeta(): Promise<boolean> {
  const retryDelaysMs = [0, 500, 1500, 3000];
  for (const retryDelay of retryDelaysMs) {
    if (retryDelay > 0) await delay(retryDelay);
    try {
      const sources = await cmd('get_sources');
      if (sources.length === 0) continue;

      sourcesCache.clear();
      enabledIds.clear();
      for (const s of sources) {
        sourcesCache.set(s.type, {
          label: s.label,
          fullName: s.name,
          colorClass: COLOR_MAP[s.color_hint] ?? DEFAULT_COLOR,
          category: s.category,
          enabled: s.enabled,
        });
        if (s.enabled) enabledIds.add(s.type);
      }
      return true;
    } catch {
      // Backend may still be warming up. Retry briefly; keep any existing cache.
    }
  }
  return sourcesCache.size > 0;
}

/**
 * Record a Settings on/off change so the filters follow it without a reload.
 * Returns false for a source the cache does not know (metadata not loaded).
 */
export function markSourceEnabled(id: string, enabled: boolean): boolean {
  const meta = sourcesCache.get(id);
  if (!meta) return false;
  meta.enabled = enabled;
  if (enabled) enabledIds.add(id);
  else enabledIds.delete(id);
  return true;
}

/**
 * The sources the user reads: stack sources plus the interests they turned
 * on. Mutated in place by `loadSourceMeta` and `markSourceEnabled`; it is the
 * default and the "reset" value of the Signal source filter.
 */
export const ENABLED_SOURCE_IDS: ReadonlySet<string> = enabledIds;

/** True only when metadata is loaded and says this source is turned off. */
export function isSourceTurnedOff(id: string): boolean {
  const meta = sourcesCache.get(id);
  return meta !== undefined && !meta.enabled;
}

export function getSourceLabel(id: string): string {
  return sourcesCache.get(id)?.label ?? id;
}
export function getSourceFullName(id: string): string {
  return sourcesCache.get(id)?.fullName ?? id;
}
export function getSourceColorClass(id: string): string {
  return sourcesCache.get(id)?.colorClass ?? DEFAULT_COLOR;
}
/** Enabled sources grouped by category. A turned-off source is never listed. */
export function getSourcesByCategory(): Map<string, string[]> {
  const groups = new Map<string, string[]>();
  for (const [id, meta] of sourcesCache) {
    if (!meta.enabled) continue;
    const cat = meta.category;
    const list = groups.get(cat) ?? [];
    list.push(id);
    groups.set(cat, list);
  }
  return groups;
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';

export type DepStatus = 'blind_spot' | 'falling_behind' | 'no_coverage' | 'well_covered';

export interface DepRow {
  name: string;
  status: DepStatus;
  urgency: Urgency;
  gap: EvidenceItem | null;
  signals: EvidenceItem[];
  projects: string[];
}

export const STATUS_CONFIG: Record<DepStatus, { labelKey: string; color: string; dot: string }> = {
  blind_spot: { labelKey: 'blindspots.status.blindSpot', color: 'text-red-400', dot: 'bg-red-400' },
  falling_behind: { labelKey: 'blindspots.status.drifting', color: 'text-yellow-400', dot: 'bg-yellow-400' },
  // Zero available signals: honest gray, not activity-yellow (2026-08-31 audit).
  no_coverage: { labelKey: 'blindspots.status.noCoverage', color: 'text-text-muted', dot: 'bg-[#8A8A8A]' },
  well_covered: { labelKey: 'blindspots.status.covered', color: 'text-green-400', dot: 'bg-green-400' },
};

export const URGENCY_COLORS: Record<Urgency, string> = {
  critical: 'text-red-400',
  high: 'text-orange-400',
  medium: 'text-yellow-400',
  watch: 'text-blue-400',
};

const SCORE_TIERS = [
  { max: 10, color: 'text-emerald-400', bg: 'bg-emerald-500', labelKey: 'blindspots.score.excellent' },
  { max: 25, color: 'text-green-400', bg: 'bg-green-500', labelKey: 'blindspots.score.good' },
  { max: 50, color: 'text-yellow-400', bg: 'bg-yellow-500', labelKey: 'blindspots.score.moderate' },
  { max: 75, color: 'text-orange-400', bg: 'bg-orange-500', labelKey: 'blindspots.score.significant' },
  { max: 100, color: 'text-red-400', bg: 'bg-red-500', labelKey: 'blindspots.score.critical' },
] as const;

export const URGENCY_ORDER: Record<Urgency, number> = { critical: 0, high: 1, medium: 2, watch: 3 };

export const MAX_SIGNALS_PER_DEP = 2;

export function getScoreTier(score: number) {
  return SCORE_TIERS.find(t => score <= t.max) ?? SCORE_TIERS[4];
}

export function extractItemId(evidenceId: string): number | null {
  const match = evidenceId.match(/(?:bs_missed_|llm-bs-)(\d+)/);
  return match ? parseInt(match[1]!, 10) : null;
}

export function depFromItem(item: EvidenceItem): string | null {
  return item.affected_deps.length > 0 ? item.affected_deps[0]! : null;
}

/**
 * Recover the bare package name from an ecosystem-qualified display name
 * ("react (npm)" -> "react"). Mirrors the backend's `bare_package_name`
 * (src-tauri/src/blind_spots.rs): gap rows carry display names while
 * missed-signal rows carry bare dep names, so any comparison between the two
 * lanes must strip the qualifier first. A name with no qualifier passes
 * through unchanged.
 */
export function barePackageName(displayName: string): string {
  const idx = displayName.lastIndexOf(' (');
  return idx >= 0 && displayName.endsWith(')') ? displayName.slice(0, idx) : displayName;
}

const SOURCE_LABELS: Record<string, { label: string; color: string }> = {
  npm_registry: { label: 'release', color: 'text-green-400/70' },
  crates_io: { label: 'release', color: 'text-green-400/70' },
  pypi: { label: 'release', color: 'text-green-400/70' },
  go_modules: { label: 'release', color: 'text-green-400/70' },
  devto: { label: 'article', color: 'text-blue-400/60' },
  hackernews: { label: 'discussion', color: 'text-orange-400/60' },
  reddit: { label: 'discussion', color: 'text-orange-400/60' },
  github: { label: 'code', color: 'text-purple-400/60' },
  lobsters: { label: 'discussion', color: 'text-orange-400/60' },
  lemmy: { label: 'discussion', color: 'text-green-400/60' },
  mastodon: { label: 'discussion', color: 'text-purple-400/60' },
  arxiv: { label: 'paper', color: 'text-cyan-400/60' },
};

export function sourceTypeLabel(source: string): { label: string; color: string } | null {
  return SOURCE_LABELS[source] ?? null;
}

export function signalMatchesDep(signal: EvidenceItem, depName: string): boolean {
  // Compare on the BARE package name: dep rows are named with the backend's
  // display name ("react (npm)") while a missed signal's affected_deps carry
  // bare names ("react"). Comparing the qualified name verbatim never matched,
  // so every react signal spawned a second bare "react" row next to
  // "react (npm)" (live audit 2026-08-31, Uncovered Dependencies).
  const lower = barePackageName(depName).toLowerCase();
  return signal.affected_deps.some(d => barePackageName(d).toLowerCase() === lower)
    || signal.title.toLowerCase().includes(lower);
}

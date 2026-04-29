// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';

export type DepStatus = 'blind_spot' | 'falling_behind' | 'well_covered';

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
  well_covered: { labelKey: 'blindspots.status.covered', color: 'text-green-400', dot: 'bg-green-400' },
};

export const URGENCY_COLORS: Record<Urgency, string> = {
  critical: 'text-red-400',
  high: 'text-orange-400',
  medium: 'text-yellow-400',
  watch: 'text-blue-400',
};

export const SCORE_TIERS = [
  { max: 25, color: 'text-green-400', bg: 'bg-green-500', labelKey: 'blindspots.score.good' },
  { max: 50, color: 'text-yellow-400', bg: 'bg-yellow-500', labelKey: 'blindspots.score.moderate' },
  { max: 75, color: 'text-orange-400', bg: 'bg-orange-500', labelKey: 'blindspots.score.significant' },
  { max: 100, color: 'text-red-400', bg: 'bg-red-500', labelKey: 'blindspots.score.critical' },
] as const;

export const URGENCY_ORDER: Record<Urgency, number> = { critical: 0, high: 1, medium: 2, watch: 3 };

export const MAX_SIGNALS_PER_DEP = 2;

export function getScoreTier(score: number) {
  return SCORE_TIERS.find(t => score <= t.max) ?? SCORE_TIERS[3];
}

export function extractItemId(evidenceId: string): number | null {
  const match = evidenceId.match(/bs_missed_(\d+)/);
  return match ? parseInt(match[1]!, 10) : null;
}

export function depFromItem(item: EvidenceItem): string | null {
  return item.affected_deps.length > 0 ? item.affected_deps[0]! : null;
}

export function signalMatchesDep(signal: EvidenceItem, depName: string): boolean {
  const lower = depName.toLowerCase();
  return signal.affected_deps.some(d => d.toLowerCase() === lower)
    || signal.title.toLowerCase().includes(lower);
}

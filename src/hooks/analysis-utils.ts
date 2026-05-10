// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { SourceRelevance } from '../types';

/**
 * Live narration event emitted during analysis.
 */
export interface NarrationEvent {
  type: string;
  message: string;
  source?: string;
  relevance?: number;
  timestamp: number;
}

// Near-miss extraction constants
const NEAR_MISS_FLOOR = 0.20;
const NEAR_MISS_LIMIT = 5;
const NEAR_MISS_RELEVANT_CEILING = 3;

/**
 * Extract near-miss items: results that almost passed the relevance threshold.
 * Returns null when there are already enough relevant results.
 */
export function extractNearMisses(
  results: SourceRelevance[],
  relevantCount: number,
): SourceRelevance[] | null {
  if (relevantCount >= NEAR_MISS_RELEVANT_CEILING) return null;
  const misses = results
    .filter((r) => !r.relevant && r.top_score >= NEAR_MISS_FLOOR)
    .sort((a, b) => b.top_score - a.top_score)
    .slice(0, NEAR_MISS_LIMIT);
  return misses.length > 0 ? misses : null;
}

/**
 * Scroll to and briefly highlight a specific item in the signals view.
 */
export function scrollToAndHighlightItem(itemId: number): void {
  setTimeout(() => {
    const el = document.querySelector(`[data-item-id="${itemId}"]`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    el?.classList.add('ring-1', 'ring-orange-500/50');
    setTimeout(() => el?.classList.remove('ring-1', 'ring-orange-500/50'), 3000);
  }, 300);
}

// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Maps analysis stages and source events to user-friendly narration
// Used exclusively by FirstRunTransition for the first-time experience

import { getSourceLabel } from '../config/sources';

/**
 * `hasProfile` is false when there is nothing to rank against yet (no
 * interests, no detected tech, no project scan). The context and relevance
 * stages must not then claim to read "your project context" or rank by "your
 * stack" (fresh-profile E2E 2026-10-09).
 */
export function getStageNarration(stage: string, hasProfile = true): string {
  switch (stage) {
    case 'init': return 'Initializing your intelligence engine...';
    case 'context': return hasProfile
      ? 'Reading your project context to personalize results...'
      : 'No project or interests yet — ranking by freshness and quality...';
    case 'fetch': return 'Reading the developer internet — pulling from your intelligence sources...';
    case 'scrape': return 'Extracting full article content for deeper analysis...';
    case 'embed': return 'Building semantic understanding of each article...';
    case 'relevance': return hasProfile
      ? 'Scoring and ranking for relevance to your stack...'
      : 'Scoring and ranking by freshness and quality...';
    case 'rerank': return 'AI is re-ranking the best matches for precision...';
    case 'complete': return 'Analysis complete — your briefing is ready!';
    default: return 'Processing...';
  }
}

export function getSourceNarration(source: string, count: number): string {
  const label = getSourceLabel(source);
  // Neutral wording: relevance isn't scored yet at fetch time, so don't claim
  // these "match your interests" — just report what arrived from each source.
  if (count === 0) return `${label} — nothing new right now`;
  return `${label} — ${count} ${count === 1 ? 'item' : 'items'} in`;
}

/**
 * No "out of N scanned" denominators: how many items were read informs no
 * action (doctrine rule 3) — the first-run overlay used to lead with a big
 * "375 ANALYZED". The relevant count is what the user acts on.
 */
export function getCelebrationMessage(relevantCount: number, profileEmpty = false): string {
  // Profileless first run: there is nothing to rank against yet, so "0 relevant"
  // is structural. Be honest about WHY and point at the one action that unlocks
  // ranking — never imply the scan underperformed.
  if (profileEmpty) {
    return '4DA ranks by what matters to you — add your stack or point it at a project folder and relevance kicks in. Until then, browse the newest reads ranked by quality.';
  }
  if (relevantCount === 0) return 'Nothing matched yet. Your profile is learning — results sharpen with use.';
  if (relevantCount <= 3) return `Found ${relevantCount} ${relevantCount === 1 ? 'item' : 'items'} tailored to your profile.`;
  if (relevantCount <= 10) return `${relevantCount} items matched your profile.`;
  return `${relevantCount} relevant items to review.`;
}

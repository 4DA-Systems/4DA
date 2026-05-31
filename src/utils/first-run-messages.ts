// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Maps analysis stages and source events to user-friendly narration
// Used exclusively by FirstRunTransition for the first-time experience

import { getSourceLabel } from '../config/sources';

export function getStageNarration(stage: string): string {
  switch (stage) {
    case 'init': return 'Initializing your intelligence engine...';
    case 'context': return 'Reading your project context to personalize results...';
    case 'fetch': return 'Reading the developer internet — pulling from your intelligence sources...';
    case 'scrape': return 'Extracting full article content for deeper analysis...';
    case 'embed': return 'Building semantic understanding of each article...';
    case 'relevance': return 'Scoring and ranking for relevance to your stack...';
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

export function getCelebrationMessage(relevantCount: number, total: number, profileEmpty = false): string {
  // Profileless first run: there is nothing to rank against yet, so "0 relevant"
  // is structural. Be honest about WHY and point at the one action that unlocks
  // ranking — never imply the scan underperformed.
  if (profileEmpty) {
    return `Scanned ${total} items across the dev internet. 4DA ranks by what matters to you — add your stack or point it at a project folder and relevance kicks in. For now, the freshest picks are ready to browse.`;
  }
  if (relevantCount === 0) return `Scanned ${total} items. Your profile is learning — results sharpen with use.`;
  if (relevantCount <= 3) return `Found ${relevantCount} items tailored to your interests.`;
  if (relevantCount <= 10) return `${relevantCount} items matched your profile out of ${total} scanned.`;
  return `${relevantCount} relevant items discovered across ${total} stories.`;
}

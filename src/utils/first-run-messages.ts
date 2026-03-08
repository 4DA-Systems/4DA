// Maps analysis stages and source events to user-friendly narration
// Used exclusively by FirstRunTransition for the first-time experience

import { getSourceLabel } from '../config/sources';

export function getStageNarration(stage: string): string {
  switch (stage) {
    case 'init': return 'Initializing your intelligence engine...';
    case 'context': return 'Reading your project context to personalize results...';
    case 'fetch': return 'Connecting to 11 intelligence sources...';
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
  if (count === 0) return `Checked ${label} — nothing new right now`;
  return `Found ${count} ${count === 1 ? 'item' : 'items'} matching your interests from ${label}`;
}

export function getCelebrationMessage(relevantCount: number, total: number): string {
  if (relevantCount === 0) return `Scanned ${total} items. Your profile is learning — results sharpen with use.`;
  if (relevantCount <= 3) return `Found ${relevantCount} items tailored to your interests.`;
  if (relevantCount <= 10) return `${relevantCount} items matched your profile out of ${total} scanned.`;
  return `${relevantCount} relevant items discovered across ${total} stories.`;
}

// Maps analysis stages and source events to user-friendly narration
// Used exclusively by FirstRunTransition for the first-time experience

import { getSourceLabel } from '../config/sources';

export function getStageNarration(stage: string): string {
  switch (stage) {
    case 'init': return 'Preparing analysis engine...';
    case 'context': return 'Loading your project context...';
    case 'fetch': return 'Gathering stories from across the internet...';
    case 'scrape': return 'Extracting article content...';
    case 'embed': return 'Understanding content semantics...';
    case 'relevance': return 'Scoring relevance against your interests...';
    case 'rerank': return 'AI is re-ranking the best matches...';
    case 'complete': return 'Analysis complete!';
    default: return 'Processing...';
  }
}

export function getSourceNarration(source: string, count: number): string {
  const label = getSourceLabel(source);
  if (count === 0) return `Checked ${label} — nothing new`;
  return `Found ${count} ${count === 1 ? 'story' : 'stories'} from ${label}`;
}

export function getCelebrationMessage(relevantCount: number, total: number): string {
  if (relevantCount === 0) return 'No highly relevant items yet — try adding more interests in Settings.';
  if (relevantCount <= 3) return `Found ${relevantCount} items tailored to your interests.`;
  if (relevantCount <= 10) return `${relevantCount} items matched your profile out of ${total} scanned.`;
  return `${relevantCount} relevant items discovered across ${total} stories.`;
}

// Score formatting and color utilities

export function formatScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

export function getScoreColor(score: number): string {
  if (score >= 0.5) return 'text-success';
  if (score >= 0.35) return 'text-accent-gold';
  return 'text-text-muted';
}

export function formatRelativeAge(isoTimestamp: string): string {
  const now = Date.now();
  const then = new Date(isoTimestamp).getTime();
  if (isNaN(then)) return '';
  const hours = Math.max(0, Math.floor((now - then) / 3_600_000));
  if (hours < 1) return '<1h';
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks}w`;
  const months = Math.floor(days / 30);
  return `${months}mo`;
}

export function getStageLabel(stage: string): string {
  switch (stage) {
    case 'init': return 'Initializing';
    case 'context': return 'Loading Context';
    case 'fetch': return 'Fetching Sources';
    case 'scrape': return 'Extracting Content';
    case 'embed': return 'Building Embeddings';
    case 'relevance': return 'Scoring Relevance';
    case 'rerank': return 'AI Re-ranking';
    case 'complete': return 'Complete';
    default: return stage;
  }
}

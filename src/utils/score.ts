// Score formatting and color utilities

export function formatScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

export function getScoreColor(score: number): string {
  if (score >= 0.5) return 'text-success';
  if (score >= 0.35) return 'text-accent-gold';
  return 'text-text-muted';
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

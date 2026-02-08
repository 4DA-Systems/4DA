// Score formatting and color utilities

export function formatScore(score: number): string {
  return (score * 100).toFixed(1) + '%';
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
    case 'fetch': return 'Fetching Stories';
    case 'scrape': return 'Scraping Content';
    case 'embed': return 'Embedding';
    case 'relevance': return 'Computing Relevance';
    case 'rerank': return 'LLM Re-ranking';
    case 'complete': return 'Complete';
    default: return stage;
  }
}

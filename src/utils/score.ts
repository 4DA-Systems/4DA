// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Score formatting and color utilities
import type { SourceRelevance } from '../types';

export function formatScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

/**
 * Returns an array of i18n keys describing the top scoring factors for a result item.
 * Used for score badge tooltips to make scoring transparent.
 */
export function getScoreFactorKeys(item: SourceRelevance): string[] {
  const factors: string[] = [];
  const b = item.score_breakdown;

  // Context-based factors
  if (b && b.context_score > 0.3) {
    factors.push('scoreTooltip.stackMatch');
  } else if (b && b.context_score > 0) {
    factors.push('scoreTooltip.partialStackMatch');
  }

  // Interest match
  if (b && b.interest_score > 0.3) {
    factors.push('scoreTooltip.interestMatch');
  }

  // Freshness (from breakdown)
  if (b && (b.freshness_mult ?? 1) > 1.0) {
    factors.push('scoreTooltip.freshContent');
  }

  // Confirmation signals
  if (b && (b.signal_count ?? 0) >= 3) {
    factors.push('scoreTooltip.multipleSignals');
  }

  // Decision window
  if (item.decision_window_match) {
    factors.push('scoreTooltip.decisionWindow');
  }

  // Dependency match
  if (b && (b.dep_match_score ?? 0) > 0) {
    factors.push('scoreTooltip.dependencyMatch');
  }

  // Learned preference (taste/feedback)
  if (b && (b.feedback_boost ?? 0) > 0) {
    factors.push('scoreTooltip.calibratedTaste');
  }

  // Topic affinity
  if (b && b.affinity_mult > 1.05) {
    factors.push('scoreTooltip.topicAffinity');
  }

  // Content quality
  if (b && (b.content_quality_mult ?? 1) > 1.0) {
    factors.push('scoreTooltip.highQuality');
  }

  // Novelty
  if (b && (b.novelty_mult ?? 1) > 1.0) {
    factors.push('scoreTooltip.novelContent');
  }

  // Serendipity
  if (item.serendipity) {
    factors.push('scoreTooltip.serendipity');
  }

  return factors;
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

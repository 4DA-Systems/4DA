// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Stack-specific celebration insights utility
// Extracted from FirstRunTransition for modularity

export interface ScanSummary {
  projects_scanned: number;
  total_dependencies: number;
  dependencies_by_ecosystem: { rust: number; npm: number; python: number; other: number };
  languages: string[];
  frameworks: string[];
  primary_stack: string;
  key_packages: string[];
  has_data: boolean;
}

export type Phase = 'preparing' | 'intelligence' | 'fetching' | 'analyzing' | 'celebrating' | 'fading';

interface RelevanceResult {
  relevant: boolean;
  title: string;
  score_breakdown?: {
    dep_match_score?: number;
    matched_deps?: string[];
    skill_gap_boost?: number;
  };
}

/** One celebration-screen insight; CelebrationState renders it in the user's language. */
export type StackInsight =
  | { kind: 'dependencies'; count: number; deps: string }
  | { kind: 'stack'; count: number; stack: string }
  | { kind: 'skillGap'; count: number };

export function buildStackInsights(
  results: RelevanceResult[],
  scanSummary: ScanSummary | null,
): StackInsight[] {
  const insights: StackInsight[] = [];

  // Count dep-matched results
  const depMatches = results.filter(r => r.relevant && r.score_breakdown?.dep_match_score && r.score_breakdown.dep_match_score > 0);
  if (depMatches.length > 0) {
    const uniqueDeps = new Set(depMatches.flatMap(r => r.score_breakdown?.matched_deps || []));
    if (uniqueDeps.size > 0) {
      const depList = Array.from(uniqueDeps).slice(0, 3).join(', ');
      insights.push({ kind: 'dependencies', count: depMatches.length, deps: depList });
    }
  }

  // Stack-specific count
  if (scanSummary?.primary_stack) {
    const stackTerms = scanSummary.primary_stack.toLowerCase().split(' + ');
    const stackMatches = results.filter(r => r.relevant && stackTerms.some(term => r.title.toLowerCase().includes(term)));
    if (stackMatches.length > 0) {
      insights.push({ kind: 'stack', count: stackMatches.length, stack: scanSummary.primary_stack });
    }
  }

  // Skill gap matches
  const gapMatches = results.filter(r => r.relevant && r.score_breakdown?.skill_gap_boost && r.score_breakdown.skill_gap_boost > 0);
  if (gapMatches.length > 0) {
    insights.push({ kind: 'skillGap', count: gapMatches.length });
  }

  return insights;
}

/**
 * Seconds left in the first analysis, for the "~N minutes remaining" line.
 *
 * The prior (from the enabled-source count) is only a guess, so it fades out
 * as real progress arrives: by 30% done the estimate is the observed rate
 * alone (elapsed * remaining / done). Never negative; 0 reads as "finishing".
 */
export function estimateRemainingSeconds(
  priorSeconds: number,
  elapsedSeconds: number,
  progress: number,
): number {
  const priorRemaining = Math.max(priorSeconds - elapsedSeconds, 0);
  const p = Math.min(Math.max(progress, 0), 1);
  if (p >= 1) return 0;
  if (p <= 0 || elapsedSeconds <= 0) return priorRemaining;
  const observedRemaining = (elapsedSeconds * (1 - p)) / p;
  const weight = Math.min(p / 0.3, 1);
  return Math.max(Math.round(weight * observedRemaining + (1 - weight) * priorRemaining), 0);
}

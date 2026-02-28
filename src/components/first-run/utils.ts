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

export function buildStackInsights(
  results: RelevanceResult[],
  scanSummary: ScanSummary | null,
): string[] {
  const insights: string[] = [];

  // Count dep-matched results
  const depMatches = results.filter(r => r.relevant && r.score_breakdown?.dep_match_score && r.score_breakdown.dep_match_score > 0);
  if (depMatches.length > 0) {
    const uniqueDeps = new Set(depMatches.flatMap(r => r.score_breakdown?.matched_deps || []));
    if (uniqueDeps.size > 0) {
      const depList = Array.from(uniqueDeps).slice(0, 3).join(', ');
      insights.push(`${depMatches.length} articles about your dependencies: ${depList}`);
    }
  }

  // Stack-specific count
  if (scanSummary?.primary_stack) {
    const stackTerms = scanSummary.primary_stack.toLowerCase().split(' + ');
    const stackMatches = results.filter(r => r.relevant && stackTerms.some(term => r.title.toLowerCase().includes(term)));
    if (stackMatches.length > 0) {
      insights.push(`${stackMatches.length} results relevant to your ${scanSummary.primary_stack} stack`);
    }
  }

  // Skill gap matches
  const gapMatches = results.filter(r => r.relevant && r.score_breakdown?.skill_gap_boost && r.score_breakdown.skill_gap_boost > 0);
  if (gapMatches.length > 0) {
    insights.push(`${gapMatches.length} items about dependencies you haven't explored yet`);
  }

  return insights;
}

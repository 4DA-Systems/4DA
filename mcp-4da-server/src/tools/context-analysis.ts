/**
 * Context Analysis Tool
 *
 * Analyze and optimize your local context for better relevance.
 * This is a SUPERPOWER - it tells you how to make 4DA smarter about you.
 *
 * With synthesize=true, uses LLM to give personalized optimization advice.
 */

import type { FourDADatabase } from "../db.js";
import { getLLMConfig, canSynthesize, synthesize, SYNTHESIS_PROMPTS } from "../llm.js";
import { createContextAnalysisCompact, type CompactResult, type ContextAnalysisKeyData } from "../output-manager.js";

export const contextAnalysisTool = {
  name: "context_analysis",
  description: `Analyze your 4DA context and get optimization recommendations.

With synthesize=true (recommended), uses AI for personalized optimization advice.

Returns:
- Current context summary (tech stack, interests, topics)
- Context quality score
- Coverage gaps (what you care about but isn't tracked)
- AI-powered personalized recommendations (when enabled)

Use this to tune 4DA for better personalization.`,
  inputSchema: {
    type: "object",
    properties: {
      include_recommendations: {
        type: "boolean",
        description: "Include optimization recommendations (default: true)",
      },
      analyze_gaps: {
        type: "boolean",
        description: "Analyze coverage gaps (default: true)",
      },
      synthesize: {
        type: "boolean",
        description: "Use AI for personalized advice (default: true if LLM configured)",
      },
      compact: {
        type: "boolean",
        description: "Return compact result with file reference (default: true for ~80% token reduction)",
      },
    },
  },
};

export interface ContextAnalysisParams {
  include_recommendations?: boolean;
  analyze_gaps?: boolean;
  synthesize?: boolean;
  compact?: boolean;
}

interface ContextQualityScore {
  overall: number;
  dimensions: {
    name: string;
    score: number;
    explanation: string;
  }[];
}

interface CoverageGap {
  area: string;
  evidence: string;
  recommendation: string;
}

interface ContextAnalysisResult {
  context_summary: {
    role: string | null;
    tech_stack_count: number;
    interests_count: number;
    domains_count: number;
    exclusions_count: number;
    detected_tech_count: number;
    active_topics_count: number;
    learned_affinities_count: number;
    anti_topics_count: number;
  };
  quality_score: ContextQualityScore;
  tech_stack: string[];
  interests: { topic: string; weight: number }[];
  top_detected_tech: { name: string; confidence: number }[];
  top_active_topics: { topic: string; weight: number }[];
  top_affinities: { topic: string; score: number }[];
  exclusions: string[];
  coverage_gaps: CoverageGap[];
  recommendations: {
    priority: "high" | "medium" | "low";
    action: string;
    impact: string;
  }[];
  recent_items_breakdown: {
    matched_by_interests: number;
    matched_by_tech: number;
    matched_by_ace: number;
    matched_by_learned: number;
    unmatched: number;
  };
  // AI-powered personalized advice
  ai_advice?: {
    personalized_recommendations: string;
    highest_impact_change: string;
    ideal_context_description: string;
    model_used: string;
  };
}

export async function executeContextAnalysis(
  db: FourDADatabase,
  params: ContextAnalysisParams
): Promise<ContextAnalysisResult | CompactResult<ContextAnalysisKeyData>> {
  const { include_recommendations = true, analyze_gaps = true } = params;
  const useCompact = params.compact !== false; // Default to compact=true

  // Check LLM availability
  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { get: (...args: unknown[]) => unknown } } }).db;
  const llmConfig = getLLMConfig(dbInstance);
  const shouldSynthesize = params.synthesize ?? canSynthesize(llmConfig);

  // Get full context
  const context = db.getUserContext(true, true);

  // Get recent items to analyze matching patterns
  const recentItems = db.getRelevantContent(0, undefined, 100, 168); // Last week

  // Calculate quality score
  const qualityScore = calculateQualityScore(context);

  // Analyze recent items breakdown
  const itemsBreakdown = analyzeItemsBreakdown(db, recentItems);

  // Find coverage gaps
  const coverageGaps = analyze_gaps ? findCoverageGaps(context, recentItems) : [];

  // Generate recommendations
  const recommendations = include_recommendations
    ? generateRecommendations(context, qualityScore, coverageGaps, itemsBreakdown)
    : [];

  const result: ContextAnalysisResult = {
    context_summary: {
      role: context.role,
      tech_stack_count: context.tech_stack.length,
      interests_count: context.interests.length,
      domains_count: context.domains.length,
      exclusions_count: context.exclusions.length,
      detected_tech_count: context.ace?.detected_tech.length || 0,
      active_topics_count: context.ace?.active_topics.length || 0,
      learned_affinities_count: context.learned?.topic_affinities.length || 0,
      anti_topics_count: context.learned?.anti_topics.length || 0,
    },
    quality_score: qualityScore,
    tech_stack: context.tech_stack,
    interests: context.interests.map(i => ({ topic: i.topic, weight: i.weight })),
    top_detected_tech: (context.ace?.detected_tech || [])
      .slice(0, 10)
      .map(t => ({ name: t.name, confidence: t.confidence })),
    top_active_topics: (context.ace?.active_topics || [])
      .slice(0, 10)
      .map(t => ({ topic: t.topic, weight: t.weight })),
    top_affinities: (context.learned?.topic_affinities || [])
      .filter(a => a.affinity_score > 0)
      .slice(0, 10)
      .map(a => ({ topic: a.topic, score: a.affinity_score })),
    exclusions: context.exclusions,
    coverage_gaps: coverageGaps,
    recommendations,
    recent_items_breakdown: itemsBreakdown,
  };

  // AI Personalized Advice - the actual superpower
  if (shouldSynthesize && canSynthesize(llmConfig)) {
    try {
      const analysisData = {
        context_summary: result.context_summary,
        quality_score: result.quality_score,
        coverage_gaps: result.coverage_gaps,
        recommendations: result.recommendations,
        recent_items_breakdown: result.recent_items_breakdown,
      };

      const synthesis = await synthesize(llmConfig, {
        system: SYNTHESIS_PROMPTS.contextAnalysis.system,
        prompt: SYNTHESIS_PROMPTS.contextAnalysis.buildPrompt(analysisData),
        max_tokens: 350,
        complexity: SYNTHESIS_PROMPTS.contextAnalysis.complexity,
      });

      // Parse the synthesis
      const lines = synthesis.synthesis.split("\n").filter(l => l.trim());
      const highestImpact = lines.find(l =>
        l.toLowerCase().includes("highest") ||
        l.toLowerCase().includes("most important") ||
        l.toLowerCase().includes("first")
      ) || lines[0] || "";

      result.ai_advice = {
        personalized_recommendations: synthesis.synthesis,
        highest_impact_change: highestImpact,
        ideal_context_description: lines[lines.length - 1] || "",
        model_used: synthesis.model_used,
      };
    } catch (error) {
      console.error("AI advice failed:", error);
    }
  }

  // Return compact or full result based on parameter
  if (useCompact) {
    return createContextAnalysisCompact(result);
  }

  return result;
}

function calculateQualityScore(context: {
  tech_stack: string[];
  interests: { topic: string }[];
  domains: string[];
  ace?: { detected_tech: unknown[]; active_topics: unknown[] };
  learned?: { topic_affinities: { affinity_score: number }[] };
}): ContextQualityScore {
  const dimensions: { name: string; score: number; explanation: string }[] = [];

  // Explicit signals (tech stack + interests)
  const explicitScore = Math.min(1, (context.tech_stack.length + context.interests.length) / 10);
  dimensions.push({
    name: "Explicit Signals",
    score: Math.round(explicitScore * 100) / 100,
    explanation: explicitScore >= 0.7
      ? "Good coverage of explicit interests and tech"
      : "Consider adding more interests and tech stack items",
  });

  // ACE coverage
  const aceScore = context.ace
    ? Math.min(1, (context.ace.detected_tech.length + context.ace.active_topics.length) / 20)
    : 0;
  dimensions.push({
    name: "ACE Detection",
    score: Math.round(aceScore * 100) / 100,
    explanation: aceScore >= 0.5
      ? "ACE is detecting your work context well"
      : aceScore > 0
        ? "ACE has some detection - consider adding more watched directories"
        : "ACE not active - add directories to watch",
  });

  // Learned preferences
  const learnedScore = context.learned
    ? Math.min(1, context.learned.topic_affinities.filter(a => a.affinity_score > 0).length / 15)
    : 0;
  dimensions.push({
    name: "Learned Preferences",
    score: Math.round(learnedScore * 100) / 100,
    explanation: learnedScore >= 0.5
      ? "Good learning from your feedback"
      : "Give more feedback to help 4DA learn your preferences",
  });

  // Diversity
  const diversity = new Set([
    ...context.tech_stack,
    ...context.interests.map(i => i.topic),
    ...context.domains,
  ]).size;
  const diversityScore = Math.min(1, diversity / 15);
  dimensions.push({
    name: "Diversity",
    score: Math.round(diversityScore * 100) / 100,
    explanation: diversityScore >= 0.6
      ? "Good topic diversity"
      : "Consider broadening your declared interests",
  });

  const overall = dimensions.reduce((sum, d) => sum + d.score, 0) / dimensions.length;

  return {
    overall: Math.round(overall * 100) / 100,
    dimensions,
  };
}

function analyzeItemsBreakdown(
  db: FourDADatabase,
  items: { id: number; source_type: string; relevance_score: number }[]
): {
  matched_by_interests: number;
  matched_by_tech: number;
  matched_by_ace: number;
  matched_by_learned: number;
  unmatched: number;
} {
  let interests = 0;
  let tech = 0;
  let ace = 0;
  let learned = 0;
  let unmatched = 0;

  // Sample up to 20 items for detailed analysis
  const sample = items.slice(0, 20);

  for (const item of sample) {
    const explanation = db.explainRelevance(item.id, item.source_type);
    if (!explanation) {
      unmatched++;
      continue;
    }

    const mc = explanation.matching_context;
    if (mc.matching_interests.length > 0) interests++;
    else if (mc.matching_tech.length > 0) tech++;
    else if (mc.matching_topics.length > 0) ace++;
    else if (mc.matching_affinities.length > 0) learned++;
    else unmatched++;
  }

  return {
    matched_by_interests: interests,
    matched_by_tech: tech,
    matched_by_ace: ace,
    matched_by_learned: learned,
    unmatched,
  };
}

function findCoverageGaps(
  context: {
    tech_stack: string[];
    interests: { topic: string }[];
    ace?: { active_topics: { topic: string }[] };
  },
  items: { title: string; content: string; relevance_score: number }[]
): CoverageGap[] {
  const gaps: CoverageGap[] = [];

  // Find high-scoring items that don't match explicit context
  const unmatchedHighScorers = items
    .filter(i => i.relevance_score >= 0.6)
    .filter(i => {
      const text = (i.title + " " + i.content).toLowerCase();
      const matchesExplicit =
        context.tech_stack.some(t => text.includes(t.toLowerCase())) ||
        context.interests.some(int => text.includes(int.topic.toLowerCase()));
      return !matchesExplicit;
    })
    .slice(0, 5);

  for (const item of unmatchedHighScorers) {
    // Extract potential topic
    const words = item.title.toLowerCase().split(/\s+/).filter(w => w.length > 4);
    const potentialTopic = words[0] || "unidentified topic";

    gaps.push({
      area: potentialTopic,
      evidence: `Item "${item.title.substring(0, 50)}..." scored ${Math.round(item.relevance_score * 100)}% without explicit interest match`,
      recommendation: `Consider adding "${potentialTopic}" as an explicit interest`,
    });
  }

  // Check for ACE gaps
  if (!context.ace || context.ace.active_topics.length === 0) {
    gaps.push({
      area: "Local Context",
      evidence: "No active topics detected from local files",
      recommendation: "Ensure ACE is scanning your active project directories",
    });
  }

  return gaps.slice(0, 5);
}

function generateRecommendations(
  context: {
    tech_stack: string[];
    interests: { topic: string }[];
    ace?: { detected_tech: unknown[]; active_topics: unknown[] };
    learned?: { topic_affinities: { affinity_score: number }[] };
  },
  quality: ContextQualityScore,
  gaps: CoverageGap[],
  breakdown: { unmatched: number }
): { priority: "high" | "medium" | "low"; action: string; impact: string }[] {
  const recommendations: { priority: "high" | "medium" | "low"; action: string; impact: string }[] = [];

  // Low quality score
  if (quality.overall < 0.5) {
    recommendations.push({
      priority: "high",
      action: "Add more explicit interests and tech stack items",
      impact: "Will significantly improve relevance scoring",
    });
  }

  // No ACE
  if (!context.ace || context.ace.detected_tech.length === 0) {
    recommendations.push({
      priority: "high",
      action: "Configure watched directories for ACE scanning",
      impact: "Enables automatic context detection from your code",
    });
  }

  // Low learned preferences
  if (!context.learned || context.learned.topic_affinities.filter(a => a.affinity_score > 0).length < 5) {
    recommendations.push({
      priority: "medium",
      action: "Provide more feedback on items (click, save, dismiss)",
      impact: "Helps 4DA learn your preferences over time",
    });
  }

  // Coverage gaps
  for (const gap of gaps.slice(0, 2)) {
    recommendations.push({
      priority: "medium",
      action: gap.recommendation,
      impact: `Would improve matching for "${gap.area}" content`,
    });
  }

  // High unmatched rate
  if (breakdown.unmatched > 10) {
    recommendations.push({
      priority: "low",
      action: "Review your exclusions - may be too aggressive",
      impact: "Could be filtering out relevant content",
    });
  }

  return recommendations;
}
